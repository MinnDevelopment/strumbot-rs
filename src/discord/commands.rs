use std::{collections::HashMap, str::FromStr, sync::Arc};

use futures::StreamExt;
use log;
use twilight_gateway::{Event, Intents, Shard};
use twilight_http::Client;
use twilight_model::{
    application::{
        command::{ChoiceCommandOptionData, CommandOption, CommandOptionChoice},
        interaction::{application_command::CommandOptionValue, Interaction},
    },
    channel::message::MessageFlags,
    gateway::payload::incoming::{InteractionCreate, Ready},
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    id::{
        marker::{GuildMarker, RoleMarker},
        Id,
    },
};

use crate::{
    config::{Config, RoleNameConfig},
    error::AsyncError,
};

pub struct Gateway {
    pub http: Arc<Client>,
    pub config: Arc<Config>,
    role_cache: HashMap<String, Id<RoleMarker>>,
}

impl Gateway {
    const INTENTS: Intents = Intents::GUILDS;

    const DEFER: InteractionResponse = InteractionResponse {
        kind: InteractionResponseType::DeferredChannelMessageWithSource,
        data: Some(InteractionResponseData {
            allowed_mentions: None,
            attachments: None,
            choices: None,
            components: None,
            content: None,
            custom_id: None,
            embeds: None,
            flags: Some(MessageFlags::EPHEMERAL),
            title: None,
            tts: None,
        }),
    };

    pub fn new(http: Arc<Client>, config: Arc<Config>) -> Self {
        Self {
            http,
            config,
            role_cache: HashMap::new(),
        }
    }

    pub async fn run(mut self) -> Result<(), AsyncError> {
        let (shard, mut events) = Shard::new(self.http.token().unwrap().into(), Self::INTENTS).await?;

        shard.start().await?;
        log::info!("Connection established");

        while let Some(event) = events.next().await {
            match event {
                Event::InteractionCreate(InteractionCreate(interaction)) => {
                    self.on_interaction(&interaction).await;
                }
                Event::Ready(e) => {
                    if !self.on_ready(&e).await {
                        break;
                    }
                }
                _ => {}
            }
        }

        shard.shutdown();
        log::info!("Connection terminated");
        Ok(())
    }

    #[inline]
    fn to_choice(name: &String) -> CommandOptionChoice {
        CommandOptionChoice::String {
            name: name.to_string(),
            value: name.to_string(),
            name_localizations: None,
        }
    }

    async fn init_roles(&mut self, config: &RoleNameConfig, guild_id: &String) -> Result<bool, AsyncError> {
        let guild_id: Id<GuildMarker> = Id::from_str(guild_id)?;
        let role_names = config.values();

        let guild = self.http.guild(guild_id).exec().await?.model().await?;
        for role in &guild.roles {
            if role_names.iter().any(|n| role.name.eq_ignore_ascii_case(n)) {
                self.role_cache.insert(role.name.to_string(), role.id);
            }
        }

        Ok(!self.role_cache.is_empty())
    }

    async fn on_ready(&mut self, event: &Ready) -> bool {
        let r = self.config.discord.role_name.clone();

        // Find role ids
        let has_roles = if let Some(ref id) = self.config.discord.guild_id.clone() {
            match self.init_roles(&r, &id).await {
                Err(e) => {
                    log::error!("Failed to initialize roles: {}", e);
                    return false;
                }
                Ok(b) => b,
            }
        } else {
            // Try iterating all guilds the bot is connected to
            let request = match self.http.current_user_guilds().exec().await {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Failed to get guilds: {}", e);
                    return false;
                }
            };

            match request.models().await {
                Err(e) => {
                    log::error!("Failed to get guilds: {}", e);
                    return false;
                }
                Ok(guilds) => {
                    let mut has_roles = false;
                    for guild in guilds {
                        has_roles |= match self.init_roles(&r, &guild.id.to_string()).await {
                            Err(e) => {
                                log::error!("Failed to initialize roles: {}", e);
                                return false;
                            }
                            Ok(b) => b,
                        }
                    }
                    has_roles
                }
            }
        };

        if !has_roles {
            return false;
        }

        let choices = r
            .values()
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(Self::to_choice)
            .collect();

        let option = CommandOption::String(ChoiceCommandOptionData {
            description: "The event role to subscribe or unsubscribe".to_string(),
            name: "role".to_string(),
            required: true,
            choices,
            ..Default::default()
        });

        let res = self
            .http
            .interaction(event.application.id)
            .create_global_command()
            .chat_input("notify", "Subscribe or unsubscribe for notifications")
            .unwrap()
            .dm_permission(false)
            .command_options(&[option])
            .unwrap()
            .exec()
            .await;

        if let Err(ref e) = res {
            log::error!("Failed to create command: {}", e);
            return false;
        } else {
            log::info!("Successfully created notify command!");
        }

        true
    }

    async fn on_interaction(&self, interaction: &Interaction) {
        let command = match interaction {
            Interaction::ApplicationCommand(command) => command,
            _ => return,
        };

        if command.data.name != "notify" {
            log::debug!("Ignoring unknown command: {}", command.data.name);
            return;
        }

        let client = self.http.interaction(interaction.application_id());
        if let Err(e) = client
            .create_response(interaction.id(), interaction.token(), &Self::DEFER)
            .exec()
            .await
        {
            log::error!("Failed to respond to interaction: {}", e);
            return;
        }

        let option = match command.data.options.iter().find(|o| o.name == "role") {
            Some(o) => o,
            None => return,
        };

        let role_name = match option.value {
            CommandOptionValue::String(ref value) => value,
            _ => return,
        };

        let role = match self.role_cache.get(role_name) {
            Some(id) => *id,
            None => return,
        };

        let guild = match command.guild_id {
            Some(id) => id,
            None => return,
        };

        let member = command.member.as_ref().expect("Command without member in a guild");
        let user_id = command.author_id().expect("Command without author id");

        let res = if member.roles.contains(&role) {
            self.http.remove_guild_member_role(guild, user_id, role).exec().await
        } else {
            self.http.add_guild_member_role(guild, user_id, role).exec().await
        };

        if let Err(e) = res {
            log::error!("Failed to update member roles: {}", e);
        } else {
            log::info!("Successfully updated member roles! Member: {} Role: {} ({})", user_id, role_name, role);
        }

        let res = client
            .create_followup(interaction.token())
            .content("Your roles have been updated!")
            .expect("Failed to create followup!")
            .exec()
            .await;

        if let Err(e) = res {
            log::error!("Failed to send followup: {}", e);
        }
    }
}
