use regex::Regex;
use serde::Deserialize;
use std::sync::Arc;
use twilight_http::{Client, request::channel::webhook::ExecuteWebhook};
use twilight_model::id::{Id, marker::WebhookMarker};

pub struct WebhookClient {
    client: Arc<Client>,
    params: WebhookParams,
}

impl WebhookClient {
    pub fn new(client: Arc<Client>, params: WebhookParams) -> Self {
        Self { client, params }
    }

    pub fn send_message<'a>(&'a self) -> ExecuteWebhook<'a> {
        let params = &self.params;
        self.client.execute_webhook(params.id, &params.token)
    }
}

#[derive(Clone)]
pub struct WebhookParams {
    pub id: Id<WebhookMarker>,
    pub token: Box<str>,
}

impl<'de> Deserialize<'de> for WebhookParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let regex =
            Regex::new(r"^https?://(?:[a-zA-Z]+\.)?discord.com/api/webhooks/([0-9]+)/([a-zA-Z0-9-_]+)$").unwrap();

        let s = String::deserialize(deserializer)?;
        let m = regex
            .captures(&s)
            .and_then(|c| Option::zip(c.get(1).map(|m| m.as_str()), c.get(2).map(|m| m.as_str())))
            .and_then(|(id, token)| Option::zip(id.parse::<u64>().ok(), Some(token)));
        match m {
            Some((id, token)) => Ok(WebhookParams {
                id: Id::new(id),
                token: token.into(),
            }),
            None => Err(serde::de::Error::custom(format!(
                "Failed to parse string using regex.\nRegex: {}\nProvided: {}",
                regex.as_str(),
                s
            ))),
        }
    }
}

impl Default for WebhookParams {
    fn default() -> Self {
        Self {
            id: Id::new(1),
            token: "".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Holder {
        url: WebhookParams,
    }

    #[test]
    fn test_parse_webhook_params() {
        let json = r#"{
            "url": "https://discord.com/api/webhooks/983342910521090131/6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD"
        }"#;
        let holder: Holder = serde_json::from_str(json).unwrap();
        let params = holder.url;
        assert_eq!(params.id, Id::new(983342910521090131));
        assert_eq!(
            params.token.as_ref(),
            "6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD"
        );
    }

    #[test]
    fn test_parse_webhook_params_invalid() {
        let json = r#"{
            "url": "https://discord-errors.com/api/webhooks/983342910521090131/6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD"
        }"#;
        assert!(serde_json::from_str::<Holder>(json).is_err());
    }
}
