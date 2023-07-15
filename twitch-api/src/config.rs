use serde::Deserialize;

const fn default_top_clips() -> u8 {
    0
}

const fn default_grace_period() -> u8 {
    2
}

#[derive(Deserialize, Default)]
pub struct TwitchConfig {
    pub client_id: Box<str>,
    pub client_secret: Box<str>,
    pub user_login: Vec<Box<str>>,
    #[serde(default = "default_top_clips")]
    pub top_clips: u8,
    #[serde(default = "default_grace_period")]
    pub offline_grace_period: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parse() {
        let file = br#"{
            "client_id": "tRSXhpTsLQtWiI7Az7HNjmFna10XTdmi",
            "client_secret": "BJW8uMosDo02LcdU25u8dC95YTVBVZmy",
            "user_login": ["Elajjaz", "distortion2"],
            "top_clips": 5
        }"#;
        let twitch: TwitchConfig = serde_json::from_slice(file).unwrap();

        assert_eq!(twitch.client_id.as_ref(), "tRSXhpTsLQtWiI7Az7HNjmFna10XTdmi");
        assert_eq!(twitch.client_secret.as_ref(), "BJW8uMosDo02LcdU25u8dC95YTVBVZmy");
        assert_eq!(twitch.user_login, vec!["Elajjaz".into(), "distortion2".into()]);
        assert_eq!(twitch.top_clips, 5);
        assert_eq!(twitch.offline_grace_period, 2);
    }
}
