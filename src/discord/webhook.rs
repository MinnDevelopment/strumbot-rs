use once_cell::sync::Lazy;
use regex::Regex;
use std::{error::Error, fmt::Display, str::FromStr};
use twilight_http::{request::channel::webhook::ExecuteWebhook, Client};
use twilight_model::id::{marker::WebhookMarker, Id};

pub struct WebhookClient {
    client: Client,
    params: WebhookParams,
}

impl WebhookClient {
    pub fn new(client: Client, params: WebhookParams) -> Self {
        Self { client, params }
    }

    pub fn send_message(&self) -> ExecuteWebhook {
        let params = &self.params;
        self.client.execute_webhook(params.id, &params.token)
    }
}

pub struct WebhookParams {
    id: Id<WebhookMarker>,
    token: String,
}

#[derive(Debug)]
pub struct ParseError {
    regex: &'static Regex,
    value: String,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to parse string using regex.\nRegex: {}\nProvided: {}",
            self.regex, self.value
        )
    }
}

impl Error for ParseError {}

impl FromStr for WebhookParams {
    type Err = Box<dyn Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        static REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^https?://(?:\w+\.)?discord.com/api/webhooks/(\d+)/([\w-]+)$").unwrap());

        if let Some(captures) = REGEX.captures(s) {
            // We can use unwrap here because the regex is well defined and constant
            let id = captures.get(1).unwrap().as_str().parse::<u64>()?;
            let token = captures.get(2).unwrap().as_str().to_string();
            Ok(WebhookParams { id: Id::new(id), token })
        } else {
            Err(Box::new(ParseError {
                regex: &REGEX,
                value: s.into(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_webhook_params() {
        let params: WebhookParams = "https://canary.discord.com/api/webhooks/983342910521090131/6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD".parse().unwrap();
        assert_eq!(params.id, Id::new(983342910521090131));
        assert_eq!(
            params.token,
            "6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD"
        );
    }

    #[test]
    fn test_parse_webhook_params_invalid() {
        let params = WebhookParams::from_str("invalid url https://canary.discord.com/api/webhooks/983342910521090131/6iwWTd-VHL7yzlJ_W1SWagLBVtTbJK8NhlMFpnjkibU5UYqjC0KgfDrTPdxUC7fdSJlD");
        assert!(params.is_err());
    }
}
