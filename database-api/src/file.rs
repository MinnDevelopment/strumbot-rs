use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use tokio::fs;

use super::*;

pub struct FileDatabase {
    root: String,
}

impl FileDatabase {
    pub const fn new(root: String) -> Self {
        FileDatabase { root }
    }

    pub async fn setup(&self) -> Result<(), std::io::Error> {
        match fs::create_dir_all(&self.root).await {
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            res => res,
        }
    }
}

#[async_trait]
impl Database for FileDatabase {
    async fn save<V>(&self, key: &str, document: &V) -> Result<(), DatabaseError>
    where
        V: Serialize + Send + Sync,
    {
        let json = serde_json::to_string(&document)?;
        // Write to a different file to avoid crash corruption
        let name = format!("{}/{}-part.json", self.root, key);
        fs::write(&name, json).await?;
        // Move it to the right name when done (atomic)
        Ok(fs::rename(&name, format!("{}/{}.json", self.root, key)).await?)
    }

    async fn read<'de, V>(&'de self, key: &str) -> Result<V, DatabaseError>
    where
        V: DeserializeOwned + Send + Sync,
    {
        let file = fs::read(format!("{}/{}.json", self.root, key)).await?;
        Ok(serde_json::from_slice(&file)?)
    }

    async fn delete(&self, key: &str) -> Result<(), DatabaseError> {
        Ok(fs::remove_file(format!("{}/{}.json", self.root, key)).await?)
    }
}
