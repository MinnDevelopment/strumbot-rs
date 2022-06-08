use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use eos::DateTime;
use log::{error, info};
use serde::{Deserialize, Serialize};
use twilight_model::http::attachment::Attachment;
use twilight_util::builder::embed::{EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder, ImageSource};

use crate::{
    config::{Config, EventName},
    discord::WebhookClient,
    error::{AsyncError as Error, RequestError},
    twitch::{Game, Stream, TwitchClient},
};

const fn split_duration(secs: u32) -> (u8, u8, u8) {
    let hour = (secs / 3600) % 60;
    let mins = (secs / 60) % 60;
    let secs = secs % 60;
    (hour as u8, mins as u8, secs as u8)
}

#[derive(Deserialize, Serialize)]
struct StreamSegment {
    game: Game,
    position: u32,
    video_id: String,
}

impl StreamSegment {
    async fn from(client: &Arc<TwitchClient>, stream: &Stream, game: &Game) -> Self {
        let position = eos::DateTime::utc_now().duration_since(&stream.started_at).as_secs() as u32;
        let video_id = match stream.get_video(client).await {
            Ok(v) => v.id,
            Err(e) => {
                error!("Failed to get video for stream: {}", e);
                "".to_string()
            }
        };

        Self {
            game: game.clone(),
            position,
            video_id,
        }
    }

    fn video_url(&self) -> String {
        format!("https://www.twitch.tv/videos/{}", self.video_id)
    }

    fn vod_link(&self) -> String {
        let (hour, min, sec) = split_duration(self.position);
        let display = format!("`{hour:02}:{min:02}:{sec:02}`");
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
    Live(Box<Stream>),
    Offline,
}

pub enum WatcherState {
    Unchanged,
    Ended,
    Updated,
}

#[derive(Deserialize, Serialize)]
pub struct StreamWatcher {
    pub user_name: String,
    user_id: String,
    config: Arc<Config>,
    segments: Vec<StreamSegment>,
    start_timestamp: DateTime,
    offline_timestamp: Option<SystemTime>, // TODO: Replace with eos type
}

impl StreamWatcher {
    pub fn new(user_name: String, config: Arc<Config>) -> Self {
        Self {
            user_name,
            user_id: "".to_string(), // initialized in go_live
            config,
            segments: Vec::new(),
            start_timestamp: DateTime::utc_now(),
            offline_timestamp: None,
        }
    }

    pub fn set_config(mut self, config: Arc<Config>) -> Self {
        self.config = config;
        self
    }

    pub async fn update(
        &mut self,
        client: &Arc<TwitchClient>,
        webhook: &Arc<WebhookClient>,
        stream: StreamUpdate,
    ) -> Result<WatcherState, Error> {
        match stream {
            StreamUpdate::Live(stream) if self.segments.is_empty() => {
                self.on_go_live(client, webhook, *stream).await?;
                Ok(WatcherState::Updated)
            }
            StreamUpdate::Live(stream) => {
                if self.on_update(client, webhook, *stream).await? {
                    Ok(WatcherState::Updated)
                } else {
                    Ok(WatcherState::Unchanged)
                }
            }
            StreamUpdate::Offline if !self.segments.is_empty() => {
                if self.on_offline(client, webhook).await? {
                    Ok(WatcherState::Ended)
                } else {
                    Ok(WatcherState::Updated)
                }
            }
            _ => Ok(WatcherState::Unchanged),
        }
    }

    async fn on_go_live(
        &mut self,
        client: &Arc<TwitchClient>,
        webhook: &Arc<WebhookClient>,
        stream: Stream,
    ) -> Result<(), Error> {
        self.offline_timestamp = None;
        self.start_timestamp = stream.started_at;
        self.user_id = stream.user_id.clone();

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
            let filename = "thumbnail.jpg".to_string();
            embed = embed.image(ImageSource::attachment(&filename)?);
            files = [Attachment::from_bytes(filename, thumbnail, 0)];
            request = request.attachments(&files)?;
        }

        let embed = embed.build();
        if let Err(err) = request.embeds(&[embed.clone()])?.exec().await {
            error!("Failed to send live event embed: {}\nEmbed: {:?}", err, embed);
        }
        Ok(())
    }

    async fn on_update(
        &mut self,
        client: &Arc<TwitchClient>,
        webhook: &Arc<WebhookClient>,
        stream: Stream,
    ) -> Result<bool, Error> {
        self.offline_timestamp = None;
        let old_game = match self.segments.last() {
            Some(seg) if seg.game.id == stream.game_id => return Ok(false),
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
            return Ok(true);
        }

        let mention = self.get_mention("update");
        let mut embed = self.create_embed(&stream, &game);
        embed = match self.segments.last() {
            Some(segs) if !segs.video_id.is_empty() => {
                embed.description(format!("Start watching at {}", segs.vod_link()))
            }
            _ => embed,
        };
        let content = format!("{} {} switched game to **{}**!", mention, stream.user_name, game.name);

        let mut request = webhook.send_message().content(&content)?;

        let thumbnail = stream.get_thumbnail(client).await;
        let files; // must have same lifetime as request
        if let Some(thumbnail) = thumbnail {
            let filename = "thumbnail.jpg".to_string();
            embed = embed.image(ImageSource::attachment(&filename)?);
            files = [Attachment::from_bytes(filename, thumbnail, 0)];
            request = request.attachments(&files)?;
        }

        let embed = embed.build();
        if let Err(err) = request.embeds(&[embed.clone()])?.exec().await {
            error!("Failed to send update event embed: {}\nEmbed: {:?}", err, embed);
        }
        Ok(true)
    }

    async fn on_offline(&mut self, client: &Arc<TwitchClient>, webhook: &Arc<WebhookClient>) -> Result<bool, Error> {
        // Check if the offline grace period is over (usually 2 minutes)
        match self.offline_timestamp {
            None => {
                let offset = Duration::from_secs(60 * self.config.twitch.offline_grace_period as u64);
                self.offline_timestamp = Some(SystemTime::now() + offset);
                return Ok(false);
            }
            Some(instant) => {
                if instant > SystemTime::now() {
                    return Ok(false);
                }
            }
        }

        info!("{} went offline.", self.user_name);

        if self.is_skipped(EventName::Vod) {
            self.segments.clear();
            self.offline_timestamp = None;
            return Ok(true);
        }

        let start_segment = self.segments.first().expect("Offline without any segments");

        let vid = start_segment.video_id.clone();
        let vod = if vid.is_empty() {
            None
        } else {
            match client.get_video_by_id(vid).await {
                Ok(vid) => Some(vid),
                Err(e) => {
                    error!("Failed to get VOD for offline stream: {}", e);
                    None
                }
            }
        };

        let mention = self.get_mention("vod");
        let mut embed = EmbedBuilder::new().color(0x6441A4);
        let content = format!("{} VOD from {}", mention, self.user_name);
        let mut request = webhook.send_message().content(&content)?;

        let files;
        embed = if let Some(video) = vod {
            if let Some(thumbnail) = video.get_thumbnail(client).await {
                let filename = "thumbnail.jpg".to_string();
                embed = embed.image(ImageSource::attachment(&filename)?);
                files = [Attachment::from_bytes(filename, thumbnail, 0)];
                request = request.attachments(&files)?;
            }

            embed
                .author(EmbedAuthorBuilder::new(video.title.clone()))
                .url(&video.url)
                .title(&video.url)
        } else {
            embed.author(EmbedAuthorBuilder::new("<Video Removed>".to_string()))
        };

        // Build the timestamp index for each segment of the stream
        let timestamps: Vec<String> = self
            .segments
            .iter()
            .map(|s| format!("{} {}", s.vod_link(), s.game.name))
            .collect();

        // Split into chunks of 1800 characters to stay below embed limits
        // TODO: Handle case with over 6k characters properly
        let mut index = vec![];
        let mut current = String::with_capacity(2000);
        for stamp in timestamps {
            if current.len() + stamp.len() > 1800 {
                index.push(current);
                current = String::with_capacity(2000);
            }

            current.push_str(&stamp);
            current.push('\n');
        }
        index.push(current);

        for part in index {
            embed = embed.field(EmbedFieldBuilder::new("Timestamps", &part).inline());
        }

        self.segments.clear();
        self.offline_timestamp = None;

        let num = self.config.twitch.top_clips.clamp(0, 5);
        if num > 0 {
            let clips = client
                .get_top_clips(self.user_id.clone(), &self.start_timestamp, num)
                .await?;
            let s: String = clips
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let limited = if c.title.len() >= 25 {
                        format!("{}...", &c.title[..25])
                    } else {
                        c.title.to_string()
                    };
                    format!(
                        "`{}.` [{} \u{1F855}]({}) \u{2022} **{}**\u{00A0}views\n",
                        i + 1,
                        limited,
                        c.url,
                        c.view_count
                    )
                })
                .collect();
            if !clips.is_empty() {
                embed = embed.field(EmbedFieldBuilder::new("Top Clips", &s));
            }
        }

        let embed = embed.build();
        if let Err(err) = request.embeds(&[embed.clone()])?.exec().await {
            error!("Failed to send vod event embed: {}\nEmbed: {:?}", err, embed);
        }
        Ok(true)
    }

    #[inline]
    async fn add_segment(&mut self, client: &Arc<TwitchClient>, stream: &Stream) -> Result<Game, RequestError> {
        let game = match stream.get_game(client).await {
            Ok(g) => g,
            Err(RequestError::Deserialize(e)) => {
                error!("Failed to deserialize game: {}", e);
                Game::empty()
            }
            Err(RequestError::NotFound(_, _)) => Game::empty(),
            err => return err,
        };

        self.segments.push(StreamSegment::from(client, stream, &game).await);
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
