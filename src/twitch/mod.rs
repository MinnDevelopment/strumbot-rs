pub use client::TwitchClient;
pub use error::TwitchError;
pub use model::*;

pub mod error;
pub mod model;
#[macro_use]
pub mod oauth;
pub mod client;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

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
