use std::collections::{HashMap, HashSet};

use discord_api::config::DiscordConfig;
use serde::Deserialize;
use tracing as log;
use twilight_http::Client;
use twilight_model::guild::{Guild, Permissions};
use twilight_model::id::{marker::GuildMarker, Id};
use twitch_api::config::TwitchConfig;

use commons::resolve;

use crate::errors::InitError;

const fn default_true() -> bool {
    true
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

    pub async fn init_roles(&mut self, client: &Client) -> anyhow::Result<()> {
        let guild = if let Some(ref id) = self.discord.guild_id {
            Self::get_guild(client, id.parse()?).await?
        } else {
            let guilds = client.current_user_guilds().limit(2)?.await?.models().await?;
            match guilds[..] {
                [ref guild] => Self::get_guild(client, guild.id).await?,
                [] => return Err(InitError::NoGuilds.into()),
                _ => return Err(InitError::TooManyGuilds.into()),
            }
        };

        self.init_roles_from_guild(client, guild).await;
        Ok(())
    }

    async fn get_guild(client: &Client, id: Id<GuildMarker>) -> anyhow::Result<Guild> {
        match client.guild(id).await {
            Ok(guild) => Ok(guild.model().await?),
            Err(err) => Err(err.into()),
        }
    }

    async fn init_roles_from_guild(&mut self, client: &Client, guild: Guild) {
        let role_name = &self.discord.role_name;
        let mut names = HashMap::with_capacity(3);
        names.insert(role_name.live.to_lowercase(), "live");
        names.insert(role_name.update.to_lowercase(), "update");
        names.insert(role_name.vod.to_lowercase(), "vod");
        let mut not_found: HashSet<&String> = names.keys().collect();

        for role in guild.roles {
            let name = &role.name.to_lowercase();
            if let Some(event) = names.get(name).copied() {
                let owned = event.to_owned();
                not_found.remove(name);
                log::info!(
                    "Found notification role for {} event: {} (id={})",
                    event,
                    role.name,
                    role.id
                );
                self.role_map.insert(owned, role.id.to_string());
            }
        }

        let guild_id = guild.id;
        for name in not_found {
            if name.is_empty() {
                continue;
            }

            let response = resolve! {
                client
                    .create_role(guild_id)
                    .name(name.as_str())
                    .mentionable(false)
                    .permissions(Permissions::empty())
            };

            match response {
                Err(err) => {
                    log::error!("Could not create roles due to error: {err:?}");
                    log::warn!("Make sure the bot has permissions to manage roles in your server. Missing: {name:?}");
                    break;
                }
                Ok(role) => {
                    let event = names.get(name).copied().unwrap().to_owned();
                    log::info!("Created role with name {name:?} for {event:?} event");
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
        let file = std::fs::read("../example-config.json").unwrap();
        let Config {
            twitch: _,
            discord: _,
            cache,
            role_map: _,
        } = serde_json::from_slice(&file).unwrap();

        assert!(!cache.enabled);
    }
}
