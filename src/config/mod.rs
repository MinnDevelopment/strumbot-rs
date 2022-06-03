use serde::Deserialize;

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

#[derive(Deserialize)]
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
