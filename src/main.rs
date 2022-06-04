use crate::{
    discord::{WebhookClient, WebhookParams},
    twitch::oauth::{ClientParams, OauthClient},
};
use config::Config;
use std::error::Error;
use twilight_http::Client;
use twilight_model::http::attachment::Attachment;
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

    let webhook_params: WebhookParams = config.discord.stream_notifications.parse()?;
    let webhook = WebhookClient::new(Client::new("".to_string()), webhook_params);

    for stream in streams {
        println!("{:?} top clips", stream.title);
        let clips = stream.get_top_clips(&client, 5).await?;
        for clip in clips {
            println!("{} - {}", clip.title, clip.url)
        }

        println!("\nAnd Video: {}", stream.get_video(&client).await?.url);

        let thumbnail = &stream.thumbnail_url;
        let image = client.get_thumbnail(thumbnail).await?;
        let attach = Attachment::from_bytes("thumbnail.jpg".into(), image, 0);

        webhook
            .send_message()
            .attachments(&[attach])?
            .exec()
            .await?;
    }

    Ok(())
}
