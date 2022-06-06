use crate::{
    discord::{WebhookClient, WebhookParams},
    twitch::oauth::{ClientParams, OauthClient},
};
use config::Config;
use futures::FutureExt;
use log::info;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::Arc,
    time::Duration,
};
use tokio::time::sleep;
use twilight_http::Client;
use twitch::TwitchClient;
use watcher::{StreamUpdate, StreamWatcher};

mod config;
mod discord;
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
    let config = Arc::new(config);

    let webhook_params: WebhookParams = config.discord.stream_notifications.parse()?;
    let webhook = WebhookClient::new(discord_client, webhook_params);

    let mut watchers = HashMap::with_capacity(config.twitch.user_login.len());
    for login in &config.twitch.user_login {
        let watcher = StreamWatcher::new(login.clone(), config.clone());
        watchers.insert(login.to_lowercase(), watcher);
    }

    // Twitch setup

    info!("Connecting to Twitch...");

    let oauth = OauthClient::new(ClientParams {
        client_id: config.twitch.client_id.to_string(),
        client_secret: config.twitch.client_secret.to_string(),
    });

    let mut client = TwitchClient::new(oauth).await?;

    info!(
        "Starting stream watchers... Listening for streams from {:?}",
        config.twitch.user_login
    );

    // TODO: Use channels and move each watcher to dedicated tokio task
    loop {
        // 1. Fetch streams in batch
        let streams = client
            .get_streams_by_login(&config.twitch.user_login)
            .await?;

        // 2. Check which streams are offline/missing
        let results: HashSet<String> = streams
            .iter()
            .map(|s| s.user_login.to_lowercase())
            .collect();

        // 3. Send updates for all currently live streams
        for stream in streams {
            if let Some(watcher) = watchers.get_mut(&stream.user_login.to_lowercase()) {
                watcher
                    .update(&client, &webhook, StreamUpdate::Live(Box::new(stream)))
                    .await?; // TODO: Handle errors
            }
        }

        // 4. Send updates for all streams that are offline
        for (login, watcher) in &mut watchers {
            if !results.contains(login) {
                watcher
                    .update(&client, &webhook, StreamUpdate::Offline)
                    .await?; // TODO: Handle errors
            }
        }

        // 5. Refresh oauth token if needed and wait 10 seconds for next poll event
        tokio::try_join!(
            client.refresh_auth(),
            sleep(Duration::from_secs(10)).map(Result::Ok)
        )?;
    }
}
