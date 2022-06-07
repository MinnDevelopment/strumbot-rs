use lazy_static::lazy_static;

use eos::fmt::{format_spec, FormatSpec};
use log::{info, warn};
use lru::LruCache;
use oauth::QueryParams;
use regex::Regex;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use super::{
    error::AuthorizationError, error::TwitchError, oauth, Clip, Error, Game, Stream, TwitchData,
    Video, VideoType,
};
use crate::{twitch::oauth::Identity, util::locked};

type DateTime = eos::DateTime<eos::Utc>;

const RFC3339: [FormatSpec<'static>; 12] = format_spec!("%Y-%m-%dT%H:%M:%SZ");

pub struct TwitchClient {
    oauth: oauth::OauthClient,
    identity: Mutex<oauth::Identity>,
    games_cache: Mutex<LruCache<String, Game>>,
}

impl TwitchClient {
    fn identity(&self) -> oauth::Identity {
        match self.identity.lock() {
            Ok(it) => it.clone(),
            Err(poison) => {
                warn!("Failed to lock identity mutex: {}", poison);
                let guard = poison.get_ref();
                Identity::clone(guard)
            }
        }
    }

    pub async fn new(oauth: oauth::OauthClient) -> Result<TwitchClient, AuthorizationError> {
        let identity = oauth.authorize().await?;
        Ok(Self {
            oauth,
            identity: Mutex::new(identity),
            games_cache: Mutex::new(LruCache::new(100)),
        })
    }

    pub async fn refresh_auth(&self) -> Result<(), AuthorizationError> {
        let identity = self.identity();
        if identity.expires_at < Instant::now() + Duration::from_secs(600) {
            info!("Refreshing oauth token...");
            let id = self.oauth.authorize().await?;
            let mut guard = self.identity.lock().unwrap();
            *guard = id;
        }
        Ok(())
    }

    pub async fn get_game_by_id(&self, id: String) -> Result<Game, Error> {
        lazy_static! {
            static ref EMPTY_GAME: Game = Game::empty();
        }

        if id.is_empty() {
            return Ok(EMPTY_GAME.clone());
        }

        let cached_game = locked(&self.games_cache, |cache| cache.get(&id).cloned());
        if let Some(game) = cached_game {
            return Ok(game);
        }

        let key = id.to_string();
        let query = build_query!("id" => id);
        let game: Game = self
            .oauth
            .get(&self.identity(), "games", query, move |b| {
                let mut body: TwitchData<Game> = serde_json::from_slice(&b)?;
                match body.data.pop() {
                    Some(game) => Ok(game),
                    None => Err(Box::new(TwitchError::NotFound("Game".to_string(), id))),
                }
            })
            .await?;

        Ok(locked(&self.games_cache, |cache| {
            cache.push(key, game.clone());
            game
        }))
    }

    pub async fn get_streams_by_login(&self, user_login: &[String]) -> Result<Vec<Stream>, Error> {
        let params = user_login
            .iter()
            .fold(QueryParams::builder(), |query, login| {
                query.param("user_login", login.to_string())
            })
            .build();

        self.oauth
            .get(&self.identity(), "streams", params, |b| {
                let body: TwitchData<Stream> = serde_json::from_slice(&b)?;
                Ok(body.data)
            })
            .await
    }

    pub async fn get_video_by_id(&self, id: String) -> Result<Video, Error> {
        let query = build_query!("id" => id);
        self.oauth
            .get(&self.identity(), "videos", query, move |b| {
                let mut body: TwitchData<Video> = serde_json::from_slice(&b)?;
                match body.data.pop() {
                    Some(video) => Ok(video),
                    None => Err(Box::new(TwitchError::NotFound("Video".to_string(), id))),
                }
            })
            .await
    }

    pub async fn get_video_by_stream(&self, stream: &Stream) -> Result<Video, Error> {
        let user_id = stream.user_id.clone();
        let query = build_query!(
            "type" => "archive",
            "first" => "5",
            "user_id" => user_id
        );

        self.oauth
            .get(&self.identity(), "videos", query, move |b| {
                let body: TwitchData<Video> = serde_json::from_slice(&b)?;
                let video = body
                    .data
                    .into_iter()
                    .filter(|v| v.kind == VideoType::Archive) // the stream vod is an archive
                    .find(|v| v.created_at >= stream.started_at); // video goes up after stream started
                match video {
                    Some(video) => Ok(video),
                    None => Err(Box::new(TwitchError::NotFound(
                        "Video".to_string(),
                        user_id,
                    ))),
                }
            })
            .await
    }

    pub async fn get_top_clips(
        &self,
        user_id: String,
        started_at: &DateTime,
        num: u8,
    ) -> Result<Vec<Clip>, Error> {
        let query = build_query!(
            "first" => "100", // twitch filters *after* limiting the number. we need to just get max and then filter
            "broadcaster_id" => user_id,
            "started_at" => started_at.format(RFC3339)
        );

        self.oauth
            .get(&self.identity(), "clips", query, move |b| {
                let body: TwitchData<Clip> = serde_json::from_slice(&b)?;
                let mut clips = body.data;
                clips.truncate(num as usize);
                Ok(clips)
            })
            .await
    }

    pub async fn get_thumbnail(&self, url: &str) -> Result<Vec<u8>, Error> {
        lazy_static! {
            static ref W: Regex = Regex::new(r"%?\{width\}").unwrap();
            static ref H: Regex = Regex::new(r"%?\{height\}").unwrap();
        }

        let full_url = H.replace(&W.replace(url, "1920"), "1080").to_string()
            + format!("?t={}", DateTime::utc_now().timestamp().as_seconds()).as_str();

        let request = self.oauth.http.get(full_url).build()?;
        let response = self.oauth.http.execute(request).await?;

        if response.status().is_success() {
            Ok(response.bytes().await?.as_ref().to_vec())
        } else if response.status().as_u16() == 404 {
            Err(Box::new(TwitchError::NotFound(
                "Thumbnail".to_string(),
                url.to_string(),
            )))
        } else {
            Err(Box::new(response.error_for_status().unwrap_err()))
        }
    }
}
