use std::{sync::Arc, time::Duration};

use log::info;
use twilight_model::http::attachment::Attachment;
use twilight_util::builder::embed::{
    EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder, ImageSource,
};

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
        let game = self.add_segment(client, &stream).await?;
        let mention = self.get_mention("live");
        let user_name = &stream.user_name;
        info!("User {} started streaming {}", user_name, game.name);

        if self.is_skipped(EventName::Live) {
            return Ok(());
        }

        let mut embed = self.create_embed(&stream, &game);
        let content = format!("{} {} is live with **{}**!", mention, user_name, game.name);

        let mut request = webhook.send_message().content(&content)?;

        let thumbnail = stream.get_thumbnail(client).await;
        let files; // must have same lifetime as request
        if let Some(thumbnail) = thumbnail {
            let filename = "thumbnail.png".to_string();
            embed = embed.image(ImageSource::attachment(&filename)?);
            files = [Attachment::from_bytes(filename, thumbnail, 0)];
            request = request.attachments(&files)?;
        }

        request.embeds(&[embed.build()])?.exec().await?;
        Ok(())
    }

    async fn on_update(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: Stream,
    ) -> Result<(), Error> {
        let old_game = match self.segments.last() {
            Some(seg) if seg.game.id == stream.game_id => return Ok(()),
            Some(seg) => seg.game.clone(), // have to clone so the borrow isn't an issue later
            None => {
                panic!("Impossible situation encountered. Stream game update without being live?")
            }
        };

        let game = self.add_segment(client, &stream).await?;
        info!(
            "User {} updated game. {} -> {}",
            stream.user_name, old_game.name, game.name
        );

        if self.is_skipped(EventName::Update) {
            return Ok(());
        }

        let mention = self.get_mention("update");
        let mut embed = self.create_embed(&stream, &game);
        let content = format!(
            "{} {} switched game to **{}**!",
            mention, stream.user_name, game.name
        );

        let mut request = webhook.send_message().content(&content)?;

        let thumbnail = stream.get_thumbnail(client).await;
        let files; // must have same lifetime as request
        if let Some(thumbnail) = thumbnail {
            let filename = "thumbnail.png".to_string();
            embed = embed.image(ImageSource::attachment(&filename)?);
            files = [Attachment::from_bytes(filename, thumbnail, 0)];
            request = request.attachments(&files)?;
        }

        request.embeds(&[embed.build()])?.exec().await?;
        Ok(())
    }

    async fn on_offline(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    async fn add_segment(&mut self, client: &TwitchClient, stream: &Stream) -> Result<Game, Error> {
        let game = stream
            .get_game(client)
            .await
            .unwrap_or_else(|_| Game::empty());
        self.segments
            .push(StreamSegment::from(client, stream, &game).await?);
        Ok(game)
    }

    #[inline]
    fn get_mention(&self, event: &str) -> String {
        self.config
            .get_role(event)
            .map(|id| format!("<@&{id}>"))
            .unwrap_or_else(|| "".to_string())
    }

    #[inline]
    fn is_skipped(&self, event: EventName) -> bool {
        !self.config.discord.enabled_events.contains(&event)
    }

    #[inline]
    fn create_embed(&self, stream: &Stream, game: &Game) -> EmbedBuilder {
        let url = format!("https://twitch.tv/{}", stream.user_name);
        let mut embed = EmbedBuilder::new()
            .author(EmbedAuthorBuilder::new(stream.title.clone()).build())
            .color(0x6441A4)
            .title(&url)
            .url(&url);

        if !game.id.is_empty() {
            embed = embed.field(EmbedFieldBuilder::new("Playing", &game.name).inline());
        }

        embed.field(
            EmbedFieldBuilder::new(
                "Started",
                format!("<t:{}:F>", stream.started_at.timestamp().as_seconds()),
            )
            .inline(),
        )
    }
}
