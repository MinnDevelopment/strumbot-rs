use serde::Deserialize;

use crate::WebhookParams;

const fn default_true() -> bool {
    true
}

#[derive(Deserialize, Default, Clone)]
pub struct RoleNameConfig {
    #[serde(default)]
    pub live: Box<str>,
    #[serde(default)]
    pub vod: Box<str>,
    #[serde(default)]
    pub update: Box<str>,
}

impl RoleNameConfig {
    pub fn values(&self) -> Vec<&str> {
        vec![&self.live, &self.vod, &self.update]
    }
}

#[derive(Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum EventName {
    #[serde(rename = "live")]
    Live,
    #[serde(rename = "vod")]
    Vod,
    #[serde(rename = "update")]
    Update,
}

#[derive(Deserialize, Default, Clone)]
pub struct DiscordConfig {
    pub token: Box<str>,
    #[serde(rename = "server_id", skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<Box<str>>,
    pub stream_notifications: WebhookParams,
    pub logging: Option<WebhookParams>,
    #[serde(default = "default_true")]
    pub show_notify_hints: bool,
    #[serde(default)]
    pub role_name: RoleNameConfig,
    pub enabled_events: Vec<EventName>,
    #[serde(default = "default_true")]
    pub enable_command: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<Box<str>>,
}

#[cfg(test)]
mod tests {
    use twilight_model::id::Id;

    use super::*;

    #[test]
    fn test_config_parse() {
        let file = br#"{
            "server_id": "81384788765712384",
            "token": "MzgwNDY1NTU1MzU1OTkyMDcw.GDPnv6.FC4xX7mQn3rPV-MkiVboQPWHrv88u4y5aS9NGc",
            "stream_notifications": "https://canary.discord.com/api/webhooks/983342910521090131/6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD",
            "avatar_url": "https://cdn.discordapp.com/avatars/86699011792191488/e43b5218e073a3ae0e9ff7504243bd32.png",
            "role_name": {
              "live": "live",
              "vod": "",
              "update": "new game"
            },
            "enabled_events": ["live", "update", "vod"],
            "enable_command": true
        }"#;

        let discord: DiscordConfig = serde_json::from_slice(file).unwrap();

        assert_eq!(discord.guild_id, Some("81384788765712384".into()));
        assert_eq!(
            discord.token.as_ref(),
            "MzgwNDY1NTU1MzU1OTkyMDcw.GDPnv6.FC4xX7mQn3rPV-MkiVboQPWHrv88u4y5aS9NGc"
        );
        assert_eq!(
            discord.avatar_url.as_deref(),
            Some("https://cdn.discordapp.com/avatars/86699011792191488/e43b5218e073a3ae0e9ff7504243bd32.png")
        );

        assert_eq!(discord.stream_notifications.id, Id::new(983342910521090131));
        assert_eq!(
            discord.stream_notifications.token.as_ref(),
            "6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD"
        );

        assert!(discord.enabled_events.contains(&EventName::Live));
        assert!(discord.enabled_events.contains(&EventName::Update));
        assert!(discord.enabled_events.contains(&EventName::Vod));

        let role_names = discord.role_name;
        assert_eq!(role_names.live.as_ref(), "live");
        assert_eq!(role_names.update.as_ref(), "new game");
        assert_eq!(role_names.vod.as_ref(), "");
    }
}
