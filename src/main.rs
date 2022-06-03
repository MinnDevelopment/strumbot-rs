use std::{error::Error, fmt::Display, str::FromStr};

use regex::Regex;
use twilight_http::{
    request::channel::webhook::{DeleteWebhook, ExecuteWebhook},
    Client,
};
use twilight_model::id::{marker::WebhookMarker, Id};
use twitch::TwitchClient;

use crate::twitch::{
    oauth::{ClientParams, OauthClient, QueryParams},
    TwitchData,
};

mod config;
mod twitch;

use config::Config;

type Async = Result<(), Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> Async {
    let config: String = tokio::fs::read_to_string("config.json").await?;
    let config: Config = serde_json::from_str(&config)?;

    let oauth = OauthClient::new(ClientParams {
        client_id: config.twitch.client_id,
        client_secret: config.twitch.client_secret,
    });

    let client = TwitchClient::new(oauth).await?;

    for login in &config.twitch.user_login {
        println!("Fetching {login}");
        let result = client.get_user_from_login(login).await?;

        println!("{result:?}")
    }

    let game = client.get_game_by_id("512724".to_string()).await?;
    println!("First {game:?}");
    let game = client.get_game_by_id("512724".to_string()).await?;
    println!("Cached {game:?}");

    Ok(())
}

struct WebhookClient {
    client: Client,
    params: WebhookParams,
}

impl WebhookClient {
    fn new(client: Client, params: WebhookParams) -> Self {
        Self { client, params }
    }

    fn send_message(&self) -> ExecuteWebhook {
        let params = &self.params;
        self.client.execute_webhook(params.id, &params.token)
    }

    #[allow(dead_code)]
    fn delete(&self) -> DeleteWebhook {
        let params = &self.params;
        self.client.delete_webhook(params.id).token(&params.token)
    }
}

struct WebhookParams {
    id: Id<WebhookMarker>,
    token: String,
}

#[derive(Debug)]
struct ParseError {
    regex: Regex,
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
        let re = Regex::new("^https://(?:\\w*).discord.com/api/webhooks/(\\d+)/(\\w+)$").unwrap();
        if let Some(captures) = re.captures(s) {
            // We can use unwrap here because the regex is well defined and constant
            let id = captures.get(1).unwrap().as_str().parse::<u64>()?;
            let token = captures.get(2).unwrap().as_str().to_string();
            Ok(WebhookParams {
                id: Id::new(id),
                token,
            })
        } else {
            Err(Box::new(ParseError {
                regex: re,
                value: s.into(),
            }))
        }
    }
}
