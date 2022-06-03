use lru::LruCache;
use oauth::QueryParams;
use tokio::sync::Mutex;

use super::{oauth, Stream};
use super::{Error, Game, TwitchData, TwitchError, User};

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
        {
            let mut cache = self.games_cache.lock().await;
            if let Some(game) = cache.get(&id) {
                return Ok(game.clone());
            }
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

        {
            let mut cache = self.games_cache.lock().await;
            cache.push(key, game.clone());
        }
        Ok(game)
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
}
