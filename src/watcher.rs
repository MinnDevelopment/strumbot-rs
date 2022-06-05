use std::{sync::Arc, time::Duration};

use eos::fmt::{format_spec, FormatSpec};
use log::info;

use crate::{
    config::{Config, EventName},
    discord::WebhookClient,
    twitch::{Error, Game, Stream, TwitchClient},
};

const fn split_duration(dur: &Duration) -> (u8, u8, u8) {
    let mut secs = dur.as_secs();
    let hour = (secs / 3600) % 60;
    let mins = (secs / 60) % 60;
    let secs = secs % 60;
    (hour as u8, mins as u8, secs as u8)
}

struct StreamSegment {
    game: Game,
    timestamp: Duration,
    video_id: String,
}

impl StreamSegment {
    async fn from(client: &TwitchClient, stream: &Stream, game: &Game) -> Result<Self, Error> {
        let duration = eos::DateTime::utc_now().duration_since(&stream.started_at);
        Ok(Self {
            game: game.clone(),
            timestamp: duration,
            video_id: stream.get_video(client).await?.id,
        })
    }

    fn video_url(&self) -> String {
        format!("https://www.twitch.tv/videos/{}", self.video_id)
    }

    fn vod_link(&self) -> String {
        let (hour, min, sec) = split_duration(&self.timestamp);
        let display = format!("{hour:02}:{min:02}:{sec:02}");
        if self.video_id.is_empty() {
            // Don't link a VOD if there is no video ID (deleted vod or streamer forgot to enable it or twitch being twitch)
            display
        } else {
            // Otherwise, hyperlink the VOD in the timestamp
            let query = format!("{hour:02}h{min:02}m{sec:02}s");
            let url = format!("{}?t={}", self.video_url(), query);
            format!("[{display}]({url})")
        }
    }
}

pub enum StreamUpdate {
    LIVE(Stream),
    OFFLINE,
}

pub struct StreamWatcher {
    user_login: String,
    config: Arc<Config>,
    segments: Vec<StreamSegment>,
    offline_timestamp: eos::DateTime<eos::Utc>,
}

impl StreamWatcher {
    pub fn new(user_login: String, config: Arc<Config>) -> Self {
        Self {
            user_login,
            config,
            segments: Vec::new(),
            offline_timestamp: eos::DateTime::utc_now(), // updated by first offline call
        }
    }

    pub async fn update(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: StreamUpdate,
    ) -> Result<(), Error> {
        // TODO: implement logic for LIVE and UPDATE
        match stream {
            StreamUpdate::OFFLINE => self.on_offline(client, webhook).await,
            StreamUpdate::LIVE(stream) if self.segments.is_empty() => {
                self.on_go_live(client, webhook, stream).await
            }
            StreamUpdate::LIVE(stream) => self.on_update(client, webhook, stream).await,
        }
    }

    async fn on_go_live(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: Stream,
    ) -> Result<(), Error> {
        let game = stream
            .get_game(client)
            .await
            .unwrap_or_else(|_| Game::empty());
        self.segments
            .push(StreamSegment::from(client, &stream, &game).await?);

        let mention = self
            .config
            .get_role("live")
            .map(|id| format!("<@&{id}>"))
            .unwrap_or_else(|| "".to_string());
        let user_name = stream.user_name;
        info!("User {} started streaming {}", user_name, game.name);

        let enabled = &self.config.discord.enabled_events;
        if !enabled.contains(&EventName::Live) {
            return Ok(());
        }

        // TODO: Embed

        webhook
            .send_message()
            .content(&format!(
                "{} {} is live with **{}**!",
                mention, user_name, game.name
            ))?
            .exec()
            .await?;
        Ok(())
    }

    async fn on_update(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: Stream,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn on_offline(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
    ) -> Result<(), Error> {
        Ok(())
    }
}
