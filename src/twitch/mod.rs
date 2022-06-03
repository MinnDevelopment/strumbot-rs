use oauth::QueryParams;
use std::collections::HashMap;

pub use error::TwitchError;
pub use model::*;
use tokio::sync::Mutex;

pub mod error;
pub mod model;
#[macro_use]
pub mod oauth;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct TwitchClient {
    oauth: oauth::OauthClient,
    identity: oauth::Identity,
    games_cache: Mutex<HashMap<String, Game>>,
}

impl TwitchClient {
    pub async fn new(oauth: oauth::OauthClient) -> Result<TwitchClient, oauth::AuthorizationError> {
        let identity = oauth.authorize().await?;
        Ok(Self {
            oauth,
            identity,
            games_cache: Mutex::new(HashMap::new()),
        })
    }

    pub async fn get_game_by_id(&self, id: String) -> Result<Game, Error> {
        {
            let cache = self.games_cache.lock().await;
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
            cache.insert(key, game.clone());
        }
        Ok(game)
    }

    /// Gets the user id for the given user login
    pub async fn get_user_from_login(&self, user_login: &String) -> Result<User, Error> {
        let query = build_query!(
            "login" => user_login
        );
        self.oauth
            .get(&self.identity, "users", query, move |s| {
                let mut body: TwitchData<User> = serde_json::from_slice(&s)?;
                match body.data.pop() {
                    Some(user) => Ok(user),
                    None => Err(Box::new(TwitchError::NotFound(
                        "User".to_string(),
                        user_login.to_string(),
                    ))),
                }
            })
            .await
    }
}

// Serde deserialization into Instant
pub(crate) mod expires_at {
    use serde::{de::Visitor, Deserializer};
    use std::time::{Duration, Instant};

    struct ExpiresAtVisitor;

    impl<'de> Visitor<'de> for ExpiresAtVisitor {
        type Value = Instant;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("seconds until expiration")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Instant::now() + Duration::from_secs(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Instant::now() + Duration::from_secs(value as u64))
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ExpiresAtVisitor)
    }
}
