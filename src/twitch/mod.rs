pub use client::TwitchClient;
pub use model::*;

pub mod model;
#[macro_use]
pub mod oauth;
pub mod client;

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

// pub(crate) mod ser_time {
//     use serde::{de::Visitor, Deserializer, Serializer};
//     use std::time::{Duration, SystemTime, UNIX_EPOCH};

//     struct TimeVisitor;

//     impl<'de> Visitor<'de> for TimeVisitor {
//         type Value = SystemTime;

//         fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//             formatter.write_str("seconds since epoch")
//         }

//         fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
//         where
//             E: serde::de::Error,
//         {
//             Ok(UNIX_EPOCH + Duration::from_secs(value))
//         }

//         fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
//         where
//             E: serde::de::Error,
//         {
//             Ok(if value < 0 {
//                 UNIX_EPOCH - Duration::from_secs(value.abs() as u64)
//             } else {
//                 UNIX_EPOCH + Duration::from_secs(value as u64)
//             })
//         }
//     }

//     pub fn serialize<S>(value: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         match value.duration_since(UNIX_EPOCH) {
//             Ok(epoch) => serializer.serialize_u64(epoch.as_secs()),
//             _ => serializer.serialize_i64(-(UNIX_EPOCH.duration_since(*value).unwrap().as_secs() as i64))
//         }
//     }

//     pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         deserializer.deserialize_any(TimeVisitor)
//     }
// }
