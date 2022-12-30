#![warn(
    clippy::string_slice,
    clippy::str_to_string,
    clippy::inefficient_to_string,
    clippy::manual_string_new,
    clippy::map_unwrap_or,
    clippy::needless_pass_by_value,
    clippy::unused_self,
    clippy::explicit_iter_loop
)]

use crate::{
    discord::{Gateway, WebhookClient},
    twitch::oauth::{ClientParams, OauthClient},
};
use config::Config;
use database::{Database, DatabaseError, FileDatabase};
use futures::FutureExt;
use hashbrown::{HashMap, HashSet};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{fs, sync::mpsc, time::sleep};
use tracing as log;
use twilight_http::Client;
use twitch::TwitchClient;
use watcher::{StreamUpdate, StreamWatcher, WatcherState};

#[macro_use]
pub mod util;
mod config;
mod database;
mod discord;
mod error;
mod twitch;
mod watcher;

type Async = Result<(), error::AsyncError>;
type Cache = FileDatabase;

#[tokio::main]
async fn main() -> Async {
    tracing_subscriber::fmt::init();

    let config: String = match tokio::fs::read_to_string("config.json").await {
        Ok(conf) => conf,
        Err(e) => {
            log::error!("Failed to read config.json: {}", e);
            return Ok(());
        }
    };

    let Ok(mut config) = serde_json::from_str::<Config>(&config) else {
        panic!("Failed to parse config.json");
    };

    let cache = Arc::new(Cache::new(".cache".into()));
    if config.cache.enabled {
        cache.setup().await?;
    }

    // Discord setup

    log::info!("Connecting to Discord...");

    let discord_client = Arc::new(Client::new(config.discord.token.to_string()));
    if let Err(e) = config.init_roles(&discord_client).await {
        log::error!("Failed to setup discord: {}", e);
        return Ok(());
    }

    let config = Arc::new(config);

    if config.discord.enable_command {
        let gateway = Gateway::new(Arc::clone(&discord_client), Arc::clone(&config));
        tokio::spawn(gateway.run());
    }

    let webhook_params = config.discord.stream_notifications.clone();
    let webhook = Arc::new(WebhookClient::new(discord_client, webhook_params));

    let mut watchers = HashMap::with_capacity(config.twitch.user_login.len());

    // Twitch setup

    log::info!("Connecting to Twitch...");

    let oauth = OauthClient::new(ClientParams {
        client_id: config.twitch.client_id.clone(),
        client_secret: config.twitch.client_secret.clone(),
    });

    let client = Arc::new(TwitchClient::new(oauth).await?);

    if config.cache.enabled {
        if let Err(err) = load_cache(&mut watchers, &config, &client, &webhook, &cache).await {
            log::error!("Could not load cache: {}", err);
        }
    }

    log::info!("Listening for streams from {:?}", config.twitch.user_login);

    loop {
        log::debug!("Fetching streams {:?}", config.twitch.user_login);
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
                let send = start_watcher(config.cache.enabled, &client, &webhook, &cache, watcher);
                push(&send, StreamUpdate::Live(Box::new(stream))).await;
                watchers.insert(name, send);
            }
        }

        log::debug!("Offline streams are: {:?}", offline);

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
    db: &Arc<Cache>,
    mut watcher: StreamWatcher,
) -> mpsc::Sender<StreamUpdate> {
    let (send, mut receive) = mpsc::channel(2);
    let twitch = Arc::clone(client);
    let webhook = Arc::clone(webhook);
    let db = Arc::clone(db);

    tokio::spawn(async move {
        let key = watcher.user_name.to_lowercase();

        let mut next_update = Instant::now();

        while let Some(event) = receive.recv().await {
            if next_update.elapsed().is_zero() {
                continue;
            }

            let result = watcher.update(&twitch, &webhook, event).await;
            match result {
                Ok(WatcherState::Ended) => {
                    break;
                }
                Err(e) => {
                    log::error!("[{}] Error when updating stream watcher: {}", key, e);
                }
                Ok(WatcherState::Updated) => {
                    if cache_enabled {
                        // Save the current watcher state to cache file
                        match db.save(&key, &watcher).await {
                            Err(DatabaseError::Io(e)) => {
                                log::error!("[{}] Failed to save cache: {}", key, e);
                            }
                            Err(DatabaseError::Serde(e)) => {
                                log::error!("[{}] Could not serialize watcher: {}", key, e);
                            }
                            Ok(_) => {}
                        }
                    }

                    // Wait a minute before updating again to avoid weird twitch api issues
                    next_update = Instant::now() + Duration::from_secs(60);
                }
                _ => {}
            }
        }

        if let Err(err) = db.delete(&key).await {
            log::error!("{} Failed to delete database entry: {}", key, err);
        }
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
    db: &Arc<Cache>,
) -> Async {
    if let Ok(data) = fs::metadata(".config").await {
        if !data.is_dir() {
            log::error!("Cannot load cache: .config is not a directory");
            return Ok(());
        }
    }

    let mut count = 0;
    for name in &config.twitch.user_login {
        let name = name.to_lowercase();
        let file = db.read::<StreamWatcher>(&name).await;

        match file {
            Err(DatabaseError::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
                log::debug!("Cache file for {} not found", name);
            }
            Err(DatabaseError::Io(err)) => {
                log::error!("Could not load cache for {name}: {}", err);
            }
            Err(DatabaseError::Serde(err)) => {
                log::warn!("Failed to parse watcher state for watcher {name:?} from cache: {}", err);
            }
            Ok(mut watcher) => {
                watcher = watcher.set_config(config.clone());
                let sender = start_watcher(true, client, webhook, db, watcher);
                watchers.insert(name, sender);
                count += 1;
            }
        }
    }

    if count > 0 {
        log::info!("Loaded {count} cached stream watchers");
    }
    Ok(())
}
