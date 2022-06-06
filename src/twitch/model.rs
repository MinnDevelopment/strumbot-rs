use serde::Deserialize;

use super::TwitchClient;

#[derive(Deserialize, Clone, Debug)]
pub struct Game {
    pub id: String,
    pub name: String,
}

impl Game {
    pub fn empty() -> Self {
        Game {
            id: "".to_string(),
            name: "".to_string(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct User {
    pub id: String,
    pub login: String,
    pub display_name: String,
    // #[serde(rename = "type")]
    // pub kind: String,
    // pub broadcaster_type: String,
    // pub description: String,
    // pub profile_image_url: String,
    // pub offline_image_url: String,
    // pub view_count: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoType {
    Archive,
    Upload,
    Highlight,
}

impl From<VideoType> for String {
    fn from(video_type: VideoType) -> Self {
        match video_type {
            VideoType::Archive => "archive".to_string(),
            VideoType::Upload => "upload".to_string(),
            VideoType::Highlight => "highlight".to_string(),
        }
    }
}

impl<'de> Deserialize<'de> for VideoType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "archive" => Ok(VideoType::Archive),
            "upload" => Ok(VideoType::Upload),
            "highlight" => Ok(VideoType::Highlight),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown video type: {}",
                s
            ))),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Video {
    pub id: String,
    pub url: String,
    pub title: String,
    pub thumbnail_url: String,
    pub view_count: i32,
    #[serde(rename = "type")]
    pub kind: VideoType,
    pub created_at: eos::DateTime,
}

impl Video {
    pub async fn get_thumbnail(&self, client: &TwitchClient) -> Option<Vec<u8>> {
        match client.get_thumbnail(&self.thumbnail_url).await {
            Ok(img) => Some(img),
            _ => None,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Clip {
    pub id: String,
    pub video_id: String,
    pub url: String,
    pub title: String,
    pub thumbnail_url: String,
    pub view_count: i32,
    pub created_at: eos::DateTime,
}

#[derive(Clone, Copy, Debug)]
pub enum StreamType {
    Live,
    None,
}

impl<'de> Deserialize<'de> for StreamType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "live" => Ok(StreamType::Live),
            _ => Ok(StreamType::None),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Stream {
    pub id: String,
    pub game_id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: StreamType,
    pub language: String,
    pub thumbnail_url: String,
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub started_at: eos::DateTime,
}

impl Stream {
    pub async fn get_game(&self, client: &TwitchClient) -> Result<Game, super::Error> {
        client.get_game_by_id(self.game_id.clone()).await
    }

    pub async fn get_video(&self, client: &TwitchClient) -> Result<Video, super::Error> {
        client.get_video_by_stream(self).await
    }

    pub async fn get_thumbnail(&self, client: &TwitchClient) -> Option<Vec<u8>> {
        match client.get_thumbnail(&self.thumbnail_url).await {
            Ok(img) => Some(img),
            _ => None,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct TwitchData<T> {
    pub data: Vec<T>,
}
