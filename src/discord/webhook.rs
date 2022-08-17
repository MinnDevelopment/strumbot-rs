use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use std::sync::Arc;
use twilight_http::{request::channel::webhook::ExecuteWebhook, Client};
use twilight_model::id::{marker::WebhookMarker, Id};

pub struct WebhookClient {
    client: Arc<Client>,
    params: WebhookParams,
}

impl WebhookClient {
    pub fn new(client: Arc<Client>, params: WebhookParams) -> Self {
        Self { client, params }
    }

    pub fn send_message(&self) -> ExecuteWebhook {
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
        static REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^https?://(?:\w+\.)?discord.com/api/webhooks/(\d+)/([\w-]+)$").unwrap());

        let s = String::deserialize(deserializer)?;
        let m = REGEX
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
                REGEX.as_str(),
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
