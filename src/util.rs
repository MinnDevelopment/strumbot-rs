use crate::error::AsyncError as Error;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{num::NonZeroU64, ops::Add};
use twilight_http::response::ResponseFuture;

#[async_trait]
pub trait ResponseResolve<T>
where
    T: DeserializeOwned + Unpin + Send,
{
    async fn resolve(self) -> Result<T, Error>;
}

#[async_trait]
impl<T> ResponseResolve<T> for ResponseFuture<T>
where
    T: DeserializeOwned + Unpin + Send,
{
    async fn resolve(self) -> Result<T, Error> {
        Ok(self.await?.model().await?)
    }
}

#[derive(Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq, Clone, Copy)]
pub struct Timestamp(NonZeroU64);

impl Timestamp {
    pub fn now() -> Self {
        unsafe {
            Self(NonZeroU64::new_unchecked(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ))
        }
    }
}

impl Add<u64> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: u64) -> Self::Output {
        unsafe { Self(NonZeroU64::new_unchecked(self.0.get() + rhs)) }
    }
}
