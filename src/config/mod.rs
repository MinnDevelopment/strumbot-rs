use std::collections::{HashMap, HashSet};

use errors::InitError;
use log::{error, info};
use serde::Deserialize;
use twilight_http::Client;
use twilight_model::{
    guild::{Guild, Permissions},
    id::{marker::GuildMarker, Id},
};

use crate::twitch::Error;
use crate::util::ResponseResolve;

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

#[derive(Deserialize)]
pub struct TwitchConfig {
    pub client_id: String,
    pub client_secret: String,
    pub user_login: Vec<String>,
    #[serde(default = "default_top_clips")]
    pub top_clips: u8,
    #[serde(default = "default_grace_period")]
    pub offline_grace_period: u8,
}

#[derive(Deserialize, Default)]
pub struct RoleNameConfig {
    #[serde(default)]
    pub live: String,
    #[serde(default)]
    pub vod: String,
    #[serde(default)]
    pub update: String,
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

#[derive(Deserialize)]
pub struct DiscordConfig {
    pub token: String,
    #[serde(rename = "server_id")]
    pub guild_id: Option<String>,
    pub stream_notifications: String,
    // pub logging: Option<String>,
    #[serde(default = "default_true")]
    pub show_notify_hints: bool,
    #[serde(default)]
    pub role_name: RoleNameConfig,
    pub enabled_events: Vec<EventName>,
}

#[derive(Deserialize)]
pub struct Config {
    pub twitch: TwitchConfig,
    pub discord: DiscordConfig,
    #[serde(default)]
    role_map: HashMap<String, String>, // map of name -> id (for mentions)
}

impl Config {
    pub fn get_role(&self, event: &str) -> Option<String> {
        self.role_map.get(event).cloned()
    }

    pub async fn init_roles(&mut self, client: &Client) -> Result<(), Error> {
        let guild = if let Some(ref id) = self.discord.guild_id {
            Self::get_guild(client, id.parse()?).await?
        } else {
            let guilds = client
                .current_user_guilds()
                .limit(2)?
                .exec()
                .await?
                .models()
                .await?;
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
            if let Some(event) = names.get(&role.name.to_lowercase()) {
                let owned = event.to_string();
                not_found.remove(&owned);
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

            info!("Creating role with name {name:?}");
            let response = client
                .create_role(guild_id)
                .name(name.as_str())
                .mentionable(false)
                .permissions(Permissions::empty())
                .exec()
                .resolve();

            if let Err(err) = response.await {
                error!("Could not create roles due to error: {:?}", err);
                info!("Make sure the bot has permissions to manage roles in your server. Missing: {:?}", name);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twitch_config_from_str() {
        let config: TwitchConfig = serde_json::from_str(
            r#"{
                "client_id": "some client id",
                "client_secret": "some client secret",
                "user_login": ["some user login"],
                "top_clips": 5,
                "offline_grace_period": 1
            }"#,
        )
        .unwrap();

        assert_eq!(config.client_id, "some client id");
        assert_eq!(config.client_secret, "some client secret");
        assert_eq!(config.user_login, vec!["some user login"]);
        assert_eq!(config.top_clips, 5);
        assert_eq!(config.offline_grace_period, 1);

        let config: TwitchConfig = serde_json::from_str(
            r#"{
                "client_id": "some client id",
                "client_secret": "some client secret",
                "user_login": ["some user login"]
            }"#,
        )
        .unwrap();

        assert_eq!(config.client_id, "some client id");
        assert_eq!(config.client_secret, "some client secret");
        assert_eq!(config.user_login, vec!["some user login"]);
        assert_eq!(config.top_clips, 0);
        assert_eq!(config.offline_grace_period, 2);
    }
}
