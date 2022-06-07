use crate::{
    discord::{WebhookClient, WebhookParams},
    twitch::oauth::{ClientParams, OauthClient},
};
use config::Config;
use futures::FutureExt;
use log::{error, info};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::Arc,
    time::Duration,
};
use tokio::{sync::mpsc, time::sleep};
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
    let webhook = Arc::new(WebhookClient::new(discord_client, webhook_params));

    let mut watchers: HashMap<String, mpsc::Sender<StreamUpdate>> =
        HashMap::with_capacity(config.twitch.user_login.len());

    // Twitch setup

    info!("Connecting to Twitch...");

    let oauth = OauthClient::new(ClientParams {
        client_id: config.twitch.client_id.to_string(),
        client_secret: config.twitch.client_secret.to_string(),
    });

    let client = Arc::new(TwitchClient::new(oauth).await?);

    info!("Listening for streams from {:?}", config.twitch.user_login);

    loop {
        // 1. Fetch streams in batch
        let streams = client
            .get_streams_by_login(&config.twitch.user_login)
            .await?;

        // 2. Check which streams are offline/missing
        let mut offline: HashSet<String> = streams
            .iter()
            .map(|s| s.user_login.to_lowercase())
            .collect();

        // 3. Send updates for all currently live streams
        for stream in streams {
            let name = stream.user_login.to_lowercase();
            offline.remove(&name);
            if let Some(send) = watchers.get_mut(&name) {
                push(send, StreamUpdate::Live(Box::new(stream))).await;
            } else {
                let send = create_watcher(&client, &webhook, &config, &name);
                push(&send, StreamUpdate::Live(Box::new(stream))).await;
                watchers.insert(name, send);
            }
        }

        // 4. Send updates for all streams that are offline
        for name in offline {
            if let Some(send) = watchers.remove(&name) {
                push(&send, StreamUpdate::Offline).await;
            }
        }

        // 5. Refresh oauth token if needed and wait 10 seconds for next poll event
        tokio::try_join!(
            client.refresh_auth(),
            sleep(Duration::from_secs(10)).map(Result::Ok)
        )?;
    }
}

fn create_watcher(
    client: &Arc<TwitchClient>,
    webhook: &Arc<WebhookClient>,
    config: &Arc<Config>,
    name: &str,
) -> mpsc::Sender<StreamUpdate> {
    let (send, mut receive) = mpsc::channel(2);
    let mut watcher = StreamWatcher::new(name.to_string(), Arc::clone(config));
    let twitch = Arc::clone(client);
    let webhook = Arc::clone(webhook);
    tokio::spawn(async move {
        while let Some(event) = receive.recv().await {
            let result = watcher.update(&twitch, &webhook, event).await;
            match result {
                Ok(b) if b => {
                    break;
                }
                Err(e) => {
                    error!("Error when updating stream watcher: {}", e);
                }
                _ => {}
            }
        }
        receive.close();
    });

    send
}

async fn push(s: &mpsc::Sender<StreamUpdate>, event: StreamUpdate) {
    if let Err(e) = s.send(event).await {
        error!("Error when sending stream update: {}", e);
    }
}
