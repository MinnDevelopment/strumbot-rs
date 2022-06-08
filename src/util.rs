use crate::error::AsyncError as Error;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::{num::NonZeroU64, sync::Mutex};
use twilight_http::response::ResponseFuture;

#[inline(always)]
pub fn locked<T, R>(lock: &Mutex<T>, f: impl FnOnce(&mut T) -> R) -> R {
    match lock.lock() {
        Ok(ref mut guard) => f(guard),
        Err(ref mut poisoned) => f(poisoned.get_mut()),
    }
}

pub fn now_unix() -> NonZeroU64 {
    unsafe {
        NonZeroU64::new_unchecked(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
    }
}

pub const fn plus(a: NonZeroU64, b: u64) -> NonZeroU64 {
    unsafe { NonZeroU64::new_unchecked(a.get() + b) }
}

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
