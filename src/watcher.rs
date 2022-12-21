use std::sync::Arc;

use eos::DateTime;
use serde::{Deserialize, Serialize};
use tracing as log;
use twilight_http::request::channel::webhook::ExecuteWebhook;
use twilight_model::{channel::message::embed::EmbedFooter, http::attachment::Attachment};
use twilight_util::builder::embed::{EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder, ImageSource};

use crate::{
    config::{Config, EventName},
    discord::WebhookClient,
    error::{AsyncError as Error, RequestError},
    twitch::{Game, Stream, TwitchClient, VideoDuration},
    util::Timestamp,
};

const fn split_duration(secs: u32) -> (u8, u8, u8) {
    let hour = (secs / 3600) % 60;
    let mins = (secs / 60) % 60;
    let secs = secs % 60;
    (hour as u8, mins as u8, secs as u8)
}

#[inline]
fn empty_str() -> Box<str> {
    "".into()
}

#[derive(Deserialize, Serialize)]
struct StreamSegment {
    game: Arc<Game>,
    position: u32,
    video_id: Box<str>,
}

impl StreamSegment {
    async fn from(client: &TwitchClient, stream: &Stream, game: Arc<Game>) -> Self {
        let position = DateTime::utc_now().duration_since(&stream.started_at).as_secs() as u32;
        let video_id = match stream.get_video(client).await {
            Ok(v) => v.id,
            Err(e) => {
                log::error!(
                    "[{}] Failed to get video for stream: {}",
                    stream.user_name.to_lowercase(),
                    e
                );
                empty_str()
            }
        };

        Self {
            game,
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
    pub user_name: Box<str>,
    user_id: Box<str>,
    stream_id: Box<str>,
    segments: Vec<StreamSegment>,
    start_timestamp: DateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    offline_timestamp: Option<Timestamp>,
    #[serde(default, skip)]
    config: Arc<Config>,
}

impl StreamWatcher {
    pub fn new(user_name: String, config: Arc<Config>) -> Self {
        Self {
            user_name: user_name.into(),
            user_id: empty_str(),   // initialized in go_live
            stream_id: empty_str(), // initialized in go_live
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
        client: &TwitchClient,
        webhook: &WebhookClient,
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
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: Stream,
    ) -> Result<(), Error> {
        self.offline_timestamp = None;
        self.start_timestamp = stream.started_at;
        self.user_id = stream.user_id.clone();
        self.stream_id = stream.id.clone();

        let segment = self.add_segment(client, &stream).await?;
        segment.position = 0;
        let game = segment.game.clone();

        let mention = self.get_mention("live");
        let user_name = &stream.user_name;
        log::info!("[{}] User started streaming {}", self.user_name, game.name);

        if self.is_skipped(EventName::Live) {
            return Ok(());
        }

        let mut embed = self.create_embed(&stream, &game);
        embed = self.set_footer(embed, &self.config.discord.role_name.live);

        let content = if game.is_empty() {
            format!("{} {} is live!", mention, user_name)
        } else {
            format!("{} {} is live with **{}**!", mention, user_name, game.name)
        };

        let request = webhook.send_message().content(&content)?;
        let thumbnail = stream.get_thumbnail(client).await;
        self.send(request, embed, thumbnail, "live").await;

        Ok(())
    }

    async fn on_update(
        &mut self,
        client: &TwitchClient,
        webhook: &WebhookClient,
        stream: Stream,
    ) -> Result<bool, Error> {
        self.offline_timestamp = None;
        let old_game = match self.segments.last() {
            Some(seg) => seg.game.clone(), // have to clone so the borrow isn't an issue later
            None => {
                panic!("Impossible situation encountered. Stream game update without being live?")
            }
        };

        let vod_change = stream.id != self.stream_id;
        let game_change = stream.game_id != old_game.id;
        let segment = if vod_change || game_change {
            // Stream has changed, so we need to update the segments
            self.add_segment(client, &stream).await?
        } else {
            // Nothing has changed, continue as usual.
            return Ok(false);
        };

        // Clone to avoid propagating mutable borrow
        let game = segment.game.clone();

        // Start from beginning of new vod
        if vod_change {
            segment.position = 0;
            self.stream_id = stream.id.clone();
        }

        // If the game didn't change, we don't need to send any announcement
        if !game_change {
            log::info!("[{}] Vod for current stream changed.", self.user_name);
            return Ok(true);
        }

        log::info!(
            "[{}] Stream changed game: {} -> {}",
            self.user_name,
            old_game.name,
            game.name
        );

        if self.is_skipped(EventName::Update) {
            return Ok(true);
        }

        let mut embed = self.create_embed(&stream, &game);
        embed = self.set_footer(embed, &self.config.discord.role_name.update);
        embed = match self.segments.last() {
            Some(segs) if !segs.video_id.is_empty() => {
                embed.description(format!("Start watching at {}", segs.vod_link()))
            }
            _ => embed,
        };

        let mention = self.get_mention("update");
        let content = format!("{} {} switched game to **{}**!", mention, stream.user_name, game.name);

        let request = webhook.send_message().content(&content)?;
        let thumbnail = stream.get_thumbnail(client).await;
        self.send(request, embed, thumbnail, "update").await;

        Ok(true)
    }

    async fn on_offline(&mut self, client: &TwitchClient, webhook: &WebhookClient) -> Result<bool, Error> {
        // Check if the offline grace period is over (usually 2 minutes)
        match self.offline_timestamp {
            None => {
                let offset = 60 * self.config.twitch.offline_grace_period as u64;
                self.offline_timestamp = Some(Timestamp::now() + offset);
                return Ok(false);
            }
            Some(instant) => {
                if instant > Timestamp::now() {
                    return Ok(false);
                }
            }
        }

        log::info!("[{}] stream went offline", self.user_name);

        if self.is_skipped(EventName::Vod) {
            self.segments.clear();
            self.offline_timestamp = None;
            return Ok(true);
        }

        let start_segment = self.segments.first().expect("Offline without any segments");

        let vid = start_segment.video_id.to_string();
        let vod = if vid.is_empty() {
            None
        } else {
            match client.get_video_by_id(vid).await {
                Ok(video) => Some(video),
                Err(e) => {
                    log::error!("[{}] Failed to get VOD for offline stream: {}", self.user_name, e);
                    None
                }
            }
        };

        let mention = self.get_mention("vod");
        let mut embed = EmbedBuilder::new().color(0x6441A4);
        embed = self.set_footer(embed, &self.config.discord.role_name.vod);

        let vods = client
            .get_videos(self.segments.iter().map(|seg| seg.video_id.to_string()).collect())
            .await
            .unwrap_or_default();
        let duration: VideoDuration = vods.iter().map(|v| v.duration).sum();

        let content = format!("{} VOD from {} [{}]", mention, self.user_name, duration);
        let request = webhook.send_message().content(&content)?;

        let thumbnail = if let Some(video) = vod {
            embed = embed
                .author(EmbedAuthorBuilder::new(video.title.to_string()))
                .url(video.url.as_ref())
                .title(video.url.as_ref());

            video.get_thumbnail(client).await
        } else {
            embed = embed.author(EmbedAuthorBuilder::new("<Video Removed>".to_string()));

            None
        };

        // Build the timestamp index for each segment of the stream
        let timestamps: Vec<String> = self
            .segments
            .iter()
            .map(|s| format!("{} {}", s.vod_link(), s.game.name))
            .collect();

        let mut index = vec![];
        let mut current = String::with_capacity(1000);
        for stamp in timestamps {
            // Split into chunks of 1000 characters to stay below embed limits
            if current.len() + stamp.len() > 1000 {
                // At most 4 chunks to not hit the limit of 6000 characters in total
                if index.len() == 3 {
                    current.push_str("...");
                    break; // pushed after loop
                }

                index.push(current);
                current = String::with_capacity(1000);
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
                .get_top_clips(self.user_id.to_string(), &self.start_timestamp, num)
                .await?;
            let s: String = clips
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let title = if c.title.len() >= 25 {
                        format!("{}...", &c.title[..25])
                    } else {
                        c.title.to_string()
                    };
                    format!(
                        "`{}.` [**{} \u{1F855}**]({}) \u{2022} **{}**\u{00A0}views\n",
                        i + 1,
                        title.replace(']', ")").replace('[', "("),
                        c.url,
                        c.view_count
                    )
                })
                .collect();
            if !clips.is_empty() {
                embed = embed.field(EmbedFieldBuilder::new("Top Clips", &s));
            }
        }

        self.send(request, embed, thumbnail, "vod").await;
        Ok(true)
    }

    async fn send<'a>(
        &self,
        mut request: ExecuteWebhook<'a>,
        mut embed: EmbedBuilder,
        thumbnail: Option<Vec<u8>>,
        context: &str,
    ) {
        const FILENAME: &str = "thumbnail.jpg";
        const INVALID_NAME: &str = "Filename for thumbnail is invalid";

        let files; // must have same lifetime as request
        if let Some(thumbnail) = thumbnail {
            embed = embed.image(ImageSource::attachment(FILENAME).expect(INVALID_NAME));
            files = [Attachment::from_bytes(FILENAME.to_string(), thumbnail, 0)];
            request = request.attachments(&files).expect(INVALID_NAME);
        }

        if let Some(url) = self.config.discord.avatar_url.as_deref() {
            request = request.avatar_url(url);
        }

        let embeds = [embed.build()];
        match request.embeds(&embeds) {
            Ok(request) => {
                if let Err(err) = request.await {
                    log::error!(
                        "[{}] Failed to send validated embed for {} event: {}",
                        self.user_name,
                        context,
                        err
                    );
                }
            }
            Err(err) => log::error!(
                "[{}] Tried to send invalid embed for {} event: {:?}\nEmbed: {:?}",
                self.user_name,
                context,
                err,
                embeds[0]
            ),
        }
    }

    #[inline]
    async fn add_segment<'a>(
        &'a mut self,
        client: &TwitchClient,
        stream: &Stream,
    ) -> Result<&'a mut StreamSegment, RequestError> {
        let game = match stream.get_game(client).await {
            Ok(g) => g,
            Err(RequestError::Deserialize(e)) => {
                log::error!("[{}] Failed to deserialize game: {}", self.user_name, e);
                Game::empty()
            }
            Err(RequestError::NotFound(_, _)) => Game::empty(),
            Err(e) => return Err(e),
        };

        let segment = StreamSegment::from(client, stream, game).await;
        self.segments.push(segment);
        Ok(self.segments.last_mut().unwrap())
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
    fn set_footer(&self, embed: EmbedBuilder, name: &str) -> EmbedBuilder {
        if !self.config.discord.show_notify_hints || name.is_empty() {
            return embed;
        }

        embed.footer(EmbedFooter {
            icon_url: None,
            proxy_icon_url: None,
            text: format!("Subscribe to notifications by typing: /notify role: {name}"),
        })
    }

    #[inline]
    fn create_embed(&self, stream: &Stream, game: &Game) -> EmbedBuilder {
        let url = format!("https://twitch.tv/{}", stream.user_name);
        let mut embed = EmbedBuilder::new()
            .author(EmbedAuthorBuilder::new(stream.title.to_string()).build())
            .color(0x6441A4)
            .title(&url)
            .url(&url);

        if !game.id.is_empty() {
            embed = embed.field(EmbedFieldBuilder::new("Playing", game.name.as_ref()).inline());
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
