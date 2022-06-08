use crate::{
    discord::{WebhookClient, WebhookParams},
    twitch::oauth::{ClientParams, OauthClient},
};
use config::Config;
use futures::FutureExt;
use log::{debug, error, info, warn};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::{fs, sync::mpsc, time::sleep};
use twilight_http::Client;
use twitch::TwitchClient;
use watcher::{StreamUpdate, StreamWatcher, WatcherState};

mod config;
mod discord;
mod error;
mod twitch;
mod util;
mod watcher;

type Async = Result<(), error::AsyncError>;

#[tokio::main]
async fn main() -> Async {
    simple_logger::init_with_level(log::Level::Info).unwrap();

    let config: String = tokio::fs::read_to_string("config.json").await?;
    let mut config: Config = serde_json::from_str(&config)?;

    let enable_cache = match tokio::fs::create_dir(".cache").await {
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => true,
        Err(err) => {
            warn!("Cannot create directory for system cache: {}", err);
            false
        }
        Ok(_) => true,
    };

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

    if enable_cache {
        if let Err(err) = load_cache(&mut watchers, &config, &client, &webhook).await {
            error!("Could not load cache: {}", err);
        }
    }

    info!("Listening for streams from {:?}", config.twitch.user_login);

    loop {
        debug!("Fetching streams {:?}", config.twitch.user_login);
        // 1. Fetch streams in batch
        let streams = client.get_streams_by_login(&config.twitch.user_login).await?;

        // 2. Check which streams are offline/missing
        let mut offline: HashSet<String> = config.twitch.user_login.iter().map(|s| s.to_lowercase()).collect();

        // 3. Send updates for all currently live streams
        for stream in streams {
            let name = stream.user_login.to_lowercase();
            offline.remove(&name);
            if let Some(send) = watchers.get_mut(&name) {
                push(send, StreamUpdate::Live(Box::new(stream))).await;
            } else {
                let watcher = StreamWatcher::new(name.to_string(), Arc::clone(&config));
                let send = start_watcher(enable_cache, &client, &webhook, watcher);
                push(&send, StreamUpdate::Live(Box::new(stream))).await;
                watchers.insert(name, send);
            }
        }

        debug!("Offline streams are: {:?}", offline);

        // 4. Send updates for all streams that are offline
        for name in offline {
            if let Some(send) = watchers.get_mut(&name) {
                if push(send, StreamUpdate::Offline).await {
                    watchers.remove(&name);
                }
            }
        }

        // 5. Refresh oauth token if needed and wait 10 seconds for next poll event
        tokio::try_join!(client.refresh_auth(), sleep(Duration::from_secs(10)).map(Result::Ok))?;
    }
}

fn start_watcher(
    cache_enabled: bool,
    client: &Arc<TwitchClient>,
    webhook: &Arc<WebhookClient>,
    mut watcher: StreamWatcher,
) -> mpsc::Sender<StreamUpdate> {
    let (send, mut receive) = mpsc::channel(2);
    let twitch = Arc::clone(client);
    let webhook = Arc::clone(webhook);
    tokio::spawn(async move {
        while let Some(event) = receive.recv().await {
            let result = watcher.update(&twitch, &webhook, event).await;
            match result {
                Ok(WatcherState::Ended) => {
                    break;
                }
                Err(e) => {
                    error!("[{}] Error when updating stream watcher: {}", watcher.user_name, e);
                }
                Ok(WatcherState::Updated) if cache_enabled => {
                    // Save the current watcher state to cache file
                    match serde_json::to_string(&watcher) {
                        Ok(json) => {
                            let result =
                                fs::write(format!(".cache/{}.json", watcher.user_name.to_lowercase()), json).await;
                            if let Err(e) = result {
                                error!("[{}] Error when writing cache: {}", watcher.user_name, e);
                            }
                        }
                        Err(err) => {
                            error!("[{}] Could not serialize watcher: {}", watcher.user_name, err);
                        }
                    }
                }
                _ => {}
            }
        }

        fs::remove_file(format!(".cache/{}.json", watcher.user_name.to_lowercase()))
            .await
            .ok();
        receive.close();
    });

    send
}

async fn push(s: &mpsc::Sender<StreamUpdate>, event: StreamUpdate) -> bool {
    s.send(event).await.is_err() // err indicates that the watcher is done
}

async fn load_cache(
    watchers: &mut HashMap<String, mpsc::Sender<StreamUpdate>>,
    config: &Arc<Config>,
    client: &Arc<TwitchClient>,
    webhook: &Arc<WebhookClient>,
) -> Async {
    if let Ok(data) = fs::metadata(".config").await {
        if !data.is_dir() {
            error!("Cannot load cache: .config is not a directory");
            return Ok(());
        }
    }

    let mut count = 0;
    for name in &config.twitch.user_login {
        let name = name.to_lowercase();
        let file = fs::read(format!(".cache/{name}.json")).await;

        match file {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                debug!("Cache file for {} not found", name);
            }
            Err(err) => {
                error!("Could not load cache file .cache/{name}.json: {}", err)
            }
            Ok(data) => {
                let mut watcher: StreamWatcher = match serde_json::from_slice(&data) {
                    Ok(w) => w,
                    Err(e) => {
                        error!("Failed to parse watcher state for watcher {name:?} from cache: {}", e);
                        continue;
                    }
                };

                watcher = watcher.set_config(config.clone());
                let sender = start_watcher(true, client, webhook, watcher);
                watchers.insert(name, sender);
                count += 1;
            }
        }
    }

    if count > 0 {
        info!("Loaded {count} cached stream watchers");
    }
    Ok(())
}
