use hashbrown::HashMap;
use std::{str::FromStr, sync::Arc};
use twilight_util::builder::command::StringBuilder;

use tracing as log;
use twilight_gateway::{Config as ShardConfig, Event, EventTypeFlags, Intents, Shard, ShardId};
use twilight_http::Client;
use twilight_model::{
    application::interaction::{application_command::CommandOptionValue, Interaction, InteractionData},
    channel::message::MessageFlags,
    gateway::payload::incoming::Ready,
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    id::{
        marker::{GuildMarker, RoleMarker},
        Id,
    },
};

use commons::resolve;

use crate::config::{DiscordConfig, RoleNameConfig};

pub struct Gateway {
    pub http: Arc<Client>,
    pub config: Arc<DiscordConfig>,
    role_cache: HashMap<String, Id<RoleMarker>>,
}

impl Gateway {
    const INTENTS: Intents = Intents::GUILDS;

    const DEFER: InteractionResponse = InteractionResponse {
        kind: InteractionResponseType::DeferredChannelMessageWithSource,
        data: Some(InteractionResponseData {
            flags: Some(MessageFlags::EPHEMERAL),
            tts: Some(false),
            allowed_mentions: None,
            attachments: None,
            choices: None,
            components: None,
            content: None,
            custom_id: None,
            embeds: None,
            title: None,
        }),
    };

    pub fn new(http: Arc<Client>, config: Arc<DiscordConfig>) -> Self {
        Self {
            http,
            config,
            role_cache: HashMap::new(),
        }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        let mut shard = Shard::with_config(
            ShardId::ONE,
            ShardConfig::builder(self.http.token().unwrap().into(), Self::INTENTS)
                .event_types(EventTypeFlags::INTERACTION_CREATE | EventTypeFlags::READY)
                .build(),
        );

        log::info!("Connection established");

        loop {
            match shard.next_event().await {
                Ok(Event::InteractionCreate(interaction)) => {
                    self.on_interaction(&interaction).await;
                }
                Ok(Event::Ready(e)) => {
                    if !self.on_ready(&e).await {
                        break;
                    }
                }
                Err(e) => {
                    log::error!(?e, "error in gateway event stream");

                    if e.is_fatal() {
                        break;
                    }
                }
                _ => {}
            }
        }

        log::info!("Connection terminated");
        Ok(())
    }

    #[inline]
    fn to_choice(name: &str) -> (&str, &str) {
        (name, name)
    }

    async fn init_roles(&mut self, config: &RoleNameConfig, guild_id: &str) -> anyhow::Result<bool> {
        let guild_id: Id<GuildMarker> = Id::from_str(guild_id)?;
        let role_names = config.values();

        let guild = resolve! { self.http.guild(guild_id) }?;
        for role in &guild.roles {
            if role_names.iter().any(|n| role.name.eq_ignore_ascii_case(n)) {
                self.role_cache.insert(role.name.to_string(), role.id);
            }
        }

        Ok(!self.role_cache.is_empty())
    }

    async fn on_ready(&mut self, event: &Ready) -> bool {
        let r = self.config.role_name.clone();

        // Find role ids
        let has_roles = if let Some(ref id) = self.config.guild_id.clone() {
            match self.init_roles(&r, id).await {
                Err(e) => {
                    log::error!("Failed to initialize roles: {}", e);
                    return false;
                }
                Ok(b) => b,
            }
        } else {
            // Try iterating all guilds the bot is connected to
            let request = match self.http.current_user_guilds().await {
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

        let choices = r.values().into_iter().filter(|s| !s.is_empty()).map(Self::to_choice);

        let option = StringBuilder::new("role", "The event role to subscribe or unsubscribe")
            .required(true)
            .choices(choices)
            .into();

        let res = self
            .http
            .interaction(event.application.id)
            .create_global_command()
            .chat_input("notify", "Subscribe or unsubscribe for notifications")
            .unwrap()
            .dm_permission(false)
            .command_options(&[option])
            .unwrap()
            .await;

        if let Err(ref e) = res {
            log::error!("Failed to create command: {}", e);
            return false;
        } else {
            log::info!("Successfully created notify command!");
        }

        true
    }

    async fn on_interaction(&self, interaction: &Interaction) -> Option<()> {
        let InteractionData::ApplicationCommand(command) = interaction.data.as_ref()? else {
            return None;
        };

        if command.name != "notify" {
            log::warn!("Ignoring unknown command: {}", command.name);
            return None;
        }

        let client = self.http.interaction(interaction.application_id);
        let r = client
            .create_response(interaction.id, &interaction.token, &Self::DEFER)
            .await;
        if let Err(e) = r {
            log::error!("Failed to respond to interaction: {}", e);
            return None;
        }

        let option = command.options.iter().find(|o| o.name == "role")?;

        let CommandOptionValue::String(ref role_name) = option.value else {
            return None;
        };

        let role = self.role_cache.get(role_name).copied()?;
        let guild = interaction.guild_id?;

        let member = interaction.member.as_ref().expect("Command without member in a guild");
        let author = interaction.author().expect("Command without author");

        let res = if member.roles.contains(&role) {
            self.http.remove_guild_member_role(guild, author.id, role).await
        } else {
            self.http.add_guild_member_role(guild, author.id, role).await
        };

        if let Err(e) = res {
            log::error!("Failed to update member roles: {}", e);
        } else {
            log::info!(
                "Successfully updated member roles! Member: {}#{} Role: {} ({})",
                author.name,
                author.discriminator(),
                role_name,
                role
            );
        }

        let res = client
            .create_followup(&interaction.token)
            .content("Your roles have been updated!")
            .expect("Failed to create followup!")
            .await;

        if let Err(e) = res {
            log::error!("Failed to send followup: {}", e);
        }

        Some(())
    }
}
