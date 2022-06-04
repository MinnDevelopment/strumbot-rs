use eos::fmt::{format_spec, FormatSpec};
use lru::LruCache;
use oauth::QueryParams;
use std::sync::Mutex;

use super::{oauth, Clip, Error, Game, Stream, TwitchData, TwitchError, User, Video, VideoType};
use crate::util::locked;

type DateTime = eos::DateTime<eos::Utc>;

const RFC3339: [FormatSpec<'static>; 12] = format_spec!("%Y-%m-%dT%H:%M:%SZ");
pub struct TwitchClient {
    oauth: oauth::OauthClient,
    identity: oauth::Identity,
    games_cache: Mutex<LruCache<String, Game>>,
}

impl TwitchClient {
    pub async fn new(oauth: oauth::OauthClient) -> Result<TwitchClient, oauth::AuthorizationError> {
        let identity = oauth.authorize().await?;
        Ok(Self {
            oauth,
            identity,
            games_cache: Mutex::new(LruCache::new(100)),
        })
    }

    pub async fn get_game_by_id(&self, id: String) -> Result<Game, Error> {
        let cached_game = locked(&self.games_cache, |cache| cache.get(&id).cloned());
        if let Some(game) = cached_game {
            return Ok(game);
        }

        let key = id.to_string();
        let query = build_query!("id" => id);
        let game: Game = self
            .oauth
            .get(&self.identity, "games", query, move |b| {
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

    /// Gets the user id for the given user login
    pub async fn get_user_from_login(&self, user_login: String) -> Result<User, Error> {
        let name = user_login.to_string();
        let query = build_query!(
            "login" => user_login
        );
        self.oauth
            .get(&self.identity, "users", query, move |s| {
                let mut body: TwitchData<User> = serde_json::from_slice(&s)?;
                match body.data.pop() {
                    Some(user) => Ok(user),
                    None => Err(Box::new(TwitchError::NotFound("User".to_string(), name))),
                }
            })
            .await
    }

    pub async fn get_streams_by_login(&self, user_login: &[String]) -> Result<Vec<Stream>, Error> {
        let params = user_login
            .iter()
            .fold(QueryParams::builder(), |query, login| {
                query.param("user_login", login.to_string())
            })
            .build();

        self.oauth
            .get(&self.identity, "streams", params, |b| {
                let body: TwitchData<Stream> = serde_json::from_slice(&b)?;
                Ok(body.data)
            })
            .await
    }

    pub async fn get_video_by_id(&self, id: String) -> Result<Video, Error> {
        let query = build_query!("id" => id);
        self.oauth
            .get(&self.identity, "videos", query, move |b| {
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
            .get(&self.identity, "videos", query, move |b| {
                let body: TwitchData<Video> = serde_json::from_slice(&b)?;
                let video = body
                    .data
                    .into_iter()
                    .filter(|v| v.kind == VideoType::Archive) // the stream vod is an archive
                    .find(|v| v.created_at.cmp(&stream.started_at).is_ge()); // video goes up after stream started
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
            .get(&self.identity, "clips", query, move |b| {
                let body: TwitchData<Clip> = serde_json::from_slice(&b)?;
                let mut clips = body.data;
                clips.truncate(num as usize);
                Ok(clips)
            })
            .await
    }
}
