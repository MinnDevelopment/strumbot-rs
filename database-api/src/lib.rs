use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

pub use error::*;
pub use file::*;

mod error;
mod file;

#[async_trait]
pub trait Database: Send + Sync {
    async fn save<V>(&self, key: &str, document: &V) -> Result<(), DatabaseError>
    where
        V: Serialize + Send + Sync;

    async fn read<V>(&self, key: &str) -> Result<V, DatabaseError>
    where
        V: DeserializeOwned + Send + Sync;

    async fn delete(&self, key: &str) -> Result<(), DatabaseError>;
}
