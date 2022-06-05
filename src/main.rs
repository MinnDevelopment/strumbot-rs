use crate::{
    discord::{WebhookClient, WebhookParams},
    twitch::oauth::{ClientParams, OauthClient},
};
use config::Config;
use log::info;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    time::Duration,
};
use twilight_http::Client;
use twitch::TwitchClient;
use watcher::{StreamUpdate, StreamWatcher};

mod config;
#[allow(dead_code)]
mod discord;
#[allow(dead_code)]
mod twitch;
mod util;
mod watcher;

pub type AsyncError = Box<dyn Error + Send + Sync>;
type Async = Result<(), AsyncError>;

#[tokio::main]
async fn main() -> Async {
    simple_logger::init_with_level(log::Level::Info).unwrap();

    let config: String = tokio::fs::read_to_string("config.json").await?;
    let mut config: Config = serde_json::from_str(&config)?;

    // Discord setup

    info!("Connecting to Discord...");

    let discord_client = Client::new(config.discord.token.to_string());
    config.init_roles(&discord_client).await?;

    let webhook_params: WebhookParams = config.discord.stream_notifications.parse()?;
    let webhook = WebhookClient::new(discord_client, webhook_params);

    let mut watchers: HashMap<String, StreamWatcher> =
        HashMap::with_capacity(config.twitch.user_login.len());
    for login in &config.twitch.user_login {
        let watcher = StreamWatcher::new(login.clone());
        watchers.insert(login.clone(), watcher);
    }

    // Twitch setup

    info!("Connecting to Twitch...");

    let oauth = OauthClient::new(ClientParams {
        client_id: config.twitch.client_id,
        client_secret: config.twitch.client_secret,
    });

    let mut client = TwitchClient::new(oauth).await?;

    info!(
        "Starting stream watchers... Listening for streams from {:?}",
        config.twitch.user_login
    );

    loop {
        // 1. Fetch streams in batch
        let streams = client
            .get_streams_by_login(&config.twitch.user_login)
            .await?;

        // 2. Check which streams are offline/missing
        let results: HashSet<String> = streams
            .iter()
            .map(|s| s.user_login.clone().to_lowercase())
            .collect();

        // 3. Send updates for all currently live streams
        for stream in streams {
            if let Some(watcher) = watchers.get_mut(&stream.user_login.clone().to_lowercase()) {
                watcher
                    .update(&client, &webhook, StreamUpdate::LIVE(stream))
                    .await?; // TODO: Handle errors
            }
        }

        // 4. Send updates for all streams that are offline
        for (login, watcher) in &mut watchers {
            if !results.contains(&login.to_lowercase()) {
                watcher
                    .update(&client, &webhook, StreamUpdate::OFFLINE)
                    .await?; // TODO: Handle errors
            }
        }

        // 5. Refresh oauth token if needed and wait 10 seconds for next poll event
        client.refresh_auth().await?;
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}
