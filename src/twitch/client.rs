use eos::fmt::{format_spec, FormatSpec};
use log::info;
use lru::LruCache;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::{
    oauth::{Identity, OauthClient, QueryParams},
    Clip, Game, Stream, TwitchData, Video, VideoType,
};
use crate::{error::RequestError, util::locked};

type DateTime = eos::DateTime<eos::Utc>;

const RFC3339: [FormatSpec<'static>; 12] = format_spec!("%Y-%m-%dT%H:%M:%SZ");

pub struct TwitchClient {
    oauth: OauthClient,
    identity: Mutex<Arc<Identity>>,
    games_cache: Mutex<LruCache<String, Arc<Game>>>,
}

impl TwitchClient {
    #[inline]
    fn identity(&self) -> Arc<Identity> {
        self.identity.lock().unwrap().clone()
    }

    pub async fn new(oauth: OauthClient) -> Result<TwitchClient, RequestError> {
        let identity = oauth.authorize().await?;
        Ok(Self {
            oauth,
            identity: Mutex::new(Arc::new(identity)),
            games_cache: Mutex::new(LruCache::new(100)),
        })
    }

    pub async fn refresh_auth(&self) -> Result<(), RequestError> {
        let identity = self.identity();
        if identity.expires_at < Instant::now() + Duration::from_secs(600) {
            info!("Refreshing oauth token...");
            let id = self.oauth.authorize().await?;
            let mut guard = self.identity.lock().unwrap();
            *guard = Arc::new(id);
        }
        Ok(())
    }

    pub async fn get_game_by_id(&self, id: String) -> Result<Arc<Game>, RequestError> {
        if id.is_empty() {
            return Ok(Game::empty());
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
                    None => Err(RequestError::NotFound("Game", id)),
                }
            })
            .await?;

        let game = Arc::new(game);

        Ok(locked(&self.games_cache, move |cache| {
            cache.push(key, game.clone());
            game
        }))
    }

    pub async fn get_streams_by_login(&self, user_login: &[String]) -> Result<Vec<Stream>, RequestError> {
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

    pub async fn get_video_by_id(&self, id: String) -> Result<Video, RequestError> {
        let query = build_query!("id" => id);
        self.oauth
            .get(&self.identity(), "videos", query, move |b| {
                let mut body: TwitchData<Video> = serde_json::from_slice(&b)?;
                match body.data.pop() {
                    Some(video) => Ok(video),
                    None => Err(RequestError::NotFound("Video", id)),
                }
            })
            .await
    }

    pub async fn get_video_by_stream(&self, stream: &Stream) -> Result<Video, RequestError> {
        let user_id = stream.user_id.to_string();
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
                    None => Err(RequestError::NotFound("Video", user_id)),
                }
            })
            .await
    }

    pub async fn get_videos(&self, mut ids: Vec<String>) -> Result<Vec<Video>, RequestError> {
        ids.dedup();
        let params = ids
            .iter()
            .fold(QueryParams::builder(), |query, id| query.param("id", id.to_string()))
            .build();

        self.oauth
            .get(&self.identity(), "videos", params, |b| {
                let body: TwitchData<Video> = serde_json::from_slice(&b)?;
                Ok(body.data)
            })
            .await
    }

    pub async fn get_top_clips(
        &self,
        user_id: String,
        started_at: &DateTime,
        num: u8,
    ) -> Result<Vec<Clip>, RequestError> {
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

    pub async fn get_thumbnail(&self, url: &str) -> Result<Vec<u8>, RequestError> {
        static W: Lazy<Regex> = Lazy::new(|| Regex::new(r"%?\{width\}").unwrap());
        static H: Lazy<Regex> = Lazy::new(|| Regex::new(r"%?\{height\}").unwrap());

        let full_url = H.replace(&W.replace(url, "1920"), "1080").to_string()
            + format!("?t={}", DateTime::utc_now().timestamp().as_seconds()).as_str();

        let request = self.oauth.http.get(full_url).build()?;
        let response = self.oauth.http.execute(request).await?;

        if response.status().is_success() {
            Ok(response.bytes().await?.as_ref().to_vec())
        } else if response.status().as_u16() == 404 {
            Err(RequestError::NotFound("Thumbnail", url.to_string()))
        } else {
            Err(RequestError::Http(response.status()))
        }
    }
}
