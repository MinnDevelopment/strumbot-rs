use crate::twitch::oauth::{ClientParams, OauthClient};
use config::Config;
use std::error::Error;
use twitch::TwitchClient;

mod config;
#[allow(dead_code)]
mod discord;
#[allow(dead_code)]
mod twitch;
mod util;

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

    // for login in &config.twitch.user_login {
    //     println!("Fetching {login}");
    //     let result = client.get_user_from_login(login.clone()).await?;

    //     println!("{result:?}")
    // }

    // let game = client.get_game_by_id("512724".to_string()).await?;
    // println!("First {game:?}");
    // let game = client.get_game_by_id("512724".to_string()).await?;
    // println!("Cached {game:?}");

    let streams = client
        .get_streams_by_login(&config.twitch.user_login)
        .await?;
    for stream in streams {
        println!("{:?} top clips", stream.title);
        let clips = client.get_top_clips(stream.user_id.clone(), &stream.started_at, 5).await?;
        for clip in clips {
            println!("{} - {}", clip.title, clip.url)
        }
    }

    Ok(())
}
