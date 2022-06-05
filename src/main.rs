use crate::{
    discord::{WebhookClient, WebhookParams},
    twitch::oauth::{ClientParams, OauthClient},
};
use config::Config;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    time::Duration,
};
use twilight_http::Client;
use twilight_model::http::attachment::Attachment;
use twitch::TwitchClient;
use watcher::{StreamUpdate, StreamWatcher};

mod config;
#[allow(dead_code)]
mod discord;
#[allow(dead_code)]
mod twitch;
mod util;
mod watcher;

type Async = Result<(), Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> Async {
    let config: String = tokio::fs::read_to_string("config.json").await?;
    let config: Config = serde_json::from_str(&config)?;

    let oauth = OauthClient::new(ClientParams {
        client_id: config.twitch.client_id,
        client_secret: config.twitch.client_secret,
    });

    let mut client = TwitchClient::new(oauth).await?;

    // for login in &config.twitch.user_login {
    //     println!("Fetching {login}");
    //     let result = client.get_user_from_login(login.clone()).await?;

    //     println!("{result:?}")
    // }

    // let game = client.get_game_by_id("512724".to_string()).await?;
    // println!("First {game:?}");
    // let game = client.get_game_by_id("512724".to_string()).await?;
    // println!("Cached {game:?}");

    // let streams = client
    //     .get_streams_by_login(&config.twitch.user_login)
    //     .await?;

    let webhook_params: WebhookParams = config.discord.stream_notifications.parse()?;
    let webhook = WebhookClient::new(Client::new("".to_string()), webhook_params);

    // for stream in streams {
    //     println!("{:?} top clips", stream.title);
    //     let clips = stream.get_top_clips(&client, 5).await?;
    //     for clip in clips {
    //         println!("{} - {}", clip.title, clip.url)
    //     }

    //     println!("\nAnd Video: {}", stream.get_video(&client).await?.url);

    //     let thumbnail = &stream.thumbnail_url;
    //     let image = client.get_thumbnail(thumbnail).await?;
    //     let attach = Attachment::from_bytes("thumbnail.jpg".into(), image, 0);

    //     webhook
    //         .send_message()
    //         .attachments(&[attach])?
    //         .exec()
    //         .await?;
    // }

    // todo: basic flow
    // - get streams
    // - run each watcher instance in sequence (borrow twitch client)
    // - refresh authorization if needed (twitch.refresh_auth())

    let mut watchers: HashMap<String, StreamWatcher> =
        HashMap::with_capacity(config.twitch.user_login.len());
    for login in &config.twitch.user_login {
        let watcher = StreamWatcher::new(login.clone());
        watchers.insert(login.clone(), watcher);
    }

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
