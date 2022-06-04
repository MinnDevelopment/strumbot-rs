use lazy_static::lazy_static;
use regex::Regex;
use std::{error::Error, fmt::Display, str::FromStr};
use twilight_http::{
    request::channel::webhook::{DeleteWebhook, ExecuteWebhook},
    Client,
};
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

    pub fn delete(&self) -> DeleteWebhook {
        let params = &self.params;
        self.client.delete_webhook(params.id).token(&params.token)
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
        lazy_static! {
            static ref REGEX: Regex =
                Regex::new(r"^https?://discord.com/api/webhooks/(\d+)/(\w+)$").unwrap();
        }

        if let Some(captures) = REGEX.captures(s) {
            // We can use unwrap here because the regex is well defined and constant
            let id = captures.get(1).unwrap().as_str().parse::<u64>()?;
            let token = captures.get(2).unwrap().as_str().to_string();
            Ok(WebhookParams {
                id: Id::new(id),
                token,
            })
        } else {
            Err(Box::new(ParseError {
                regex: &REGEX,
                value: s.into(),
            }))
        }
    }
}
