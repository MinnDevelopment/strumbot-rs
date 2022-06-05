use crate::AsyncError as Error;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::sync::Mutex;
use twilight_http::response::ResponseFuture;

#[inline(always)]
pub fn locked<T, R>(lock: &Mutex<T>, f: impl FnOnce(&mut T) -> R) -> R {
    match lock.lock() {
        Ok(ref mut guard) => f(guard),
        Err(ref mut poisoned) => f(poisoned.get_mut()),
    }
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
