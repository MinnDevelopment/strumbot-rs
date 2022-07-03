use std::collections::{HashMap, HashSet};

use errors::InitError;
use log::{error, info, warn};
use serde::Deserialize;
use twilight_http::Client;
use twilight_model::{
    guild::{Guild, Permissions},
    id::{marker::GuildMarker, Id},
};

use crate::util::ResponseResolve;
use crate::{discord::WebhookParams, error::AsyncError as Error};

pub mod errors;

const fn default_top_clips() -> u8 {
    0
}

const fn default_grace_period() -> u8 {
    2
}

const fn default_true() -> bool {
    true
}

#[derive(Deserialize, Default)]
pub struct TwitchConfig {
    pub client_id: String,
    pub client_secret: String,
    pub user_login: Vec<String>,
    #[serde(default = "default_top_clips")]
    pub top_clips: u8,
    #[serde(default = "default_grace_period")]
    pub offline_grace_period: u8,
}

#[derive(Deserialize, Default, Clone)]
pub struct RoleNameConfig {
    #[serde(default)]
    pub live: String,
    #[serde(default)]
    pub vod: String,
    #[serde(default)]
    pub update: String,
}

impl RoleNameConfig {
    pub fn values(&self) -> Vec<&String> {
        vec![&self.live, &self.vod, &self.update]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect()
    }
}

#[derive(Deserialize, PartialEq, Eq)]
pub enum EventName {
    #[serde(rename = "live")]
    Live,
    #[serde(rename = "vod")]
    Vod,
    #[serde(rename = "update")]
    Update,
}

#[derive(Deserialize, Default)]
pub struct DiscordConfig {
    pub token: String,
    #[serde(rename = "server_id", skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<String>,
    pub stream_notifications: WebhookParams,
    pub logging: Option<WebhookParams>,
    #[serde(default = "default_true")]
    pub show_notify_hints: bool,
    #[serde(default)]
    pub role_name: RoleNameConfig,
    pub enabled_events: Vec<EventName>,
    #[serde(default = "default_true")]
    pub enable_command: bool,
}

#[derive(Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        CacheConfig { enabled: true }
    }
}

#[derive(Deserialize, Default)]
pub struct Config {
    pub twitch: TwitchConfig,
    pub discord: DiscordConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    role_map: HashMap<String, String>, // map of event -> id (for mentions)
}

impl Config {
    pub fn get_role(&self, event: &str) -> Option<String> {
        self.role_map.get(event).cloned()
    }

    pub async fn init_roles(&mut self, client: &Client) -> Result<(), Error> {
        let guild = if let Some(ref id) = self.discord.guild_id {
            Self::get_guild(client, id.parse()?).await?
        } else {
            let guilds = client.current_user_guilds().limit(2)?.exec().await?.models().await?;
            match guilds[..] {
                [ref guild] => Self::get_guild(client, guild.id).await?,
                [] => return Err(Box::new(InitError::NoGuilds)),
                _ => return Err(Box::new(InitError::TooManyGuilds)),
            }
        };

        self.init_roles_from_guild(client, guild).await;
        Ok(())
    }

    async fn get_guild(client: &Client, id: Id<GuildMarker>) -> Result<Guild, Error> {
        match client.guild(id).exec().await {
            Ok(guild) => Ok(guild.model().await?),
            Err(err) => Err(Box::new(err)),
        }
    }

    async fn init_roles_from_guild(&mut self, client: &Client, guild: Guild) {
        let role_name = &self.discord.role_name;
        let mut names = HashMap::with_capacity(3);
        names.insert(role_name.live.to_string().to_lowercase(), "live");
        names.insert(role_name.update.to_string().to_lowercase(), "update");
        names.insert(role_name.vod.to_string().to_lowercase(), "vod");
        let mut not_found: HashSet<&String> = names.keys().collect();

        for role in guild.roles {
            let name = &role.name.to_lowercase();
            if let Some(event) = names.get(name) {
                let owned = event.to_string();
                not_found.remove(name);
                info!(
                    "Found notification role for {} event: {} (id={})",
                    event, role.name, role.id
                );
                self.role_map.insert(owned, role.id.to_string());
            }
        }

        let guild_id = guild.id;
        for name in not_found {
            if name.is_empty() {
                continue;
            }

            let response = client
                .create_role(guild_id)
                .name(name.as_str())
                .mentionable(false)
                .permissions(Permissions::empty())
                .exec()
                .resolve();

            match response.await {
                Err(err) => {
                    error!("Could not create roles due to error: {err:?}");
                    warn!("Make sure the bot has permissions to manage roles in your server. Missing: {name:?}");
                    break;
                }
                Ok(role) => {
                    let event = names.get(name).unwrap().to_string();
                    info!("Created role with name {name:?} for {event:?} event");
                    self.role_map.insert(event, role.id.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parse() {
        let file = std::fs::read("example-config.json").unwrap();
        let Config {
            twitch,
            discord,
            cache,
            role_map: _,
        } = serde_json::from_slice(&file).unwrap();

        assert_eq!(twitch.client_id, "tRSXhpTsLQtWiI7Az7HNjmFna10XTdmi");
        assert_eq!(twitch.client_secret, "BJW8uMosDo02LcdU25u8dC95YTVBVZmy");
        assert_eq!(twitch.user_login, vec!["Elajjaz", "distortion2"]);
        assert_eq!(twitch.top_clips, 5);
        assert_eq!(twitch.offline_grace_period, 2);

        assert_eq!(discord.guild_id, Some("81384788765712384".into()));
        assert_eq!(
            discord.token,
            "MzgwNDY1NTU1MzU1OTkyMDcw.GDPnv6.FC4xX7mQn3rPV-MkiVboQPWHrv88u4y5aS9NGc"
        );

        assert_eq!(discord.stream_notifications.id, Id::new(983342910521090131));
        assert_eq!(
            discord.stream_notifications.token,
            "6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD"
        );

        assert!(discord.enabled_events.contains(&EventName::Live));
        assert!(discord.enabled_events.contains(&EventName::Update));
        assert!(discord.enabled_events.contains(&EventName::Vod));

        let role_names = discord.role_name;
        assert_eq!(role_names.live, "live");
        assert_eq!(role_names.update, "new game");
        assert_eq!(role_names.vod, "");

        assert!(!cache.enabled);
    }
}
