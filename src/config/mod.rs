use serde::Deserialize;

const fn default_top_clips() -> u8 {
    0
}

const fn default_grace_period() -> u8 {
    2
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

#[derive(Deserialize)]
pub struct Config {
    pub twitch: TwitchConfig,
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
