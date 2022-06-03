use std::collections::HashMap;
use oauth::QueryParams;

pub use error::TwitchError;
pub use model::*;

pub mod error;
pub mod model;
#[macro_use]
pub mod oauth;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct TwitchClient {
    oauth: oauth::OauthClient,
    identity: oauth::Identity,
    games_cache: HashMap<String, Game>,
}

impl TwitchClient {
    pub async fn new(oauth: oauth::OauthClient) -> Result<TwitchClient, oauth::AuthorizationError> {
        let identity = oauth.authorize().await?;
        Ok(Self {
            oauth,
            identity,
            games_cache: HashMap::new(),
        })
    }

    /// Gets the user id for the given user login
    pub async fn get_user_from_login(&self, user_login: &String) -> Result<User, Error> {
        let query = build_query!(
            "login" => user_login
        );
        self.oauth
            .get(&self.identity, "users", query, move |s| {
                let body: TwitchData<User> = serde_json::from_slice(&s)?;
                match body.data.first() {
                    Some(user) => Ok(user.to_owned()),
                    None => Err(Box::new(TwitchError::UserNotFound(user_login.to_string()))),
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
