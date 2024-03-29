use std::{fmt::Display, iter::Sum, ops::Add, sync::Arc};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::TwitchClient;
use crate::error::RequestError;

static EMPTY: Lazy<Arc<Game>> = Lazy::new(|| {
    Arc::new(Game {
        id: String::new().into_boxed_str(),
        name: "No Category".to_owned().into_boxed_str(),
    })
});

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Game {
    pub id: Box<str>,
    pub name: Box<str>,
}

impl Game {
    #[inline]
    pub fn empty() -> Arc<Self> {
        Arc::clone(&EMPTY)
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.id.is_empty()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct User {
    pub id: Box<str>,
    pub login: Box<str>,
    pub display_name: Box<str>,
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
            VideoType::Archive => "archive".to_owned(),
            VideoType::Upload => "upload".to_owned(),
            VideoType::Highlight => "highlight".to_owned(),
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
            _ => Err(serde::de::Error::custom(format!("Unknown video type: {}", s))),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Video {
    pub id: Box<str>,
    pub url: Box<str>,
    pub title: Box<str>,
    pub thumbnail_url: Box<str>,
    pub view_count: i32,
    #[serde(rename = "type")]
    pub kind: VideoType,
    pub created_at: eos::DateTime,
    pub duration: VideoDuration,
}

impl Video {
    pub async fn get_thumbnail(&self, client: &TwitchClient) -> Option<Vec<u8>> {
        if self.thumbnail_url.is_empty() {
            None
        } else {
            client.get_thumbnail(&self.thumbnail_url).await.ok()
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Clip {
    pub id: Box<str>,
    pub video_id: Box<str>,
    pub url: Box<str>,
    pub title: Box<str>,
    pub thumbnail_url: Box<str>,
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
    pub id: Box<str>,
    pub game_id: Box<str>,
    pub title: Box<str>,
    #[serde(rename = "type")]
    pub kind: StreamType,
    pub language: Box<str>,
    pub thumbnail_url: Box<str>,
    pub user_id: Box<str>,
    pub user_login: Box<str>,
    pub user_name: Box<str>,
    pub started_at: eos::DateTime,
}

impl Stream {
    pub async fn get_game(&self, client: &TwitchClient) -> Result<Arc<Game>, RequestError> {
        client.get_game_by_id(self.game_id.to_string()).await
    }

    pub async fn get_video(&self, client: &TwitchClient) -> Result<Video, RequestError> {
        client.get_video_by_stream(self).await
    }

    pub async fn get_thumbnail(&self, client: &TwitchClient) -> Option<Vec<u8>> {
        client.get_thumbnail(&self.thumbnail_url).await.ok()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct TwitchData<T> {
    pub data: Vec<T>,
}

#[derive(Clone, Copy, Debug)]
pub struct VideoDuration(u32);

impl Add<VideoDuration> for VideoDuration {
    type Output = VideoDuration;

    fn add(self, other: VideoDuration) -> Self::Output {
        VideoDuration(self.0 + other.0)
    }
}

impl Sum for VideoDuration {
    fn sum<I: Iterator<Item = VideoDuration>>(iter: I) -> Self {
        iter.fold(VideoDuration(0), |acc, x| acc + x)
    }
}

impl Display for VideoDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let seconds = self.0 % 60;
        let minutes = self.0 / 60 % 60;
        let hours = self.0 / 3600;
        write!(f, "{hours:02}h{minutes:02}m{seconds:02}s")
    }
}

impl<'de> Deserialize<'de> for VideoDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        static REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"([0-9]+)([hms])").unwrap());

        let s = String::deserialize(deserializer)?;
        let duration = REGEX
            .captures_iter(&s)
            .filter_map(|m| m[1].parse::<u32>().ok().zip(m[2].bytes().next()))
            .map(|(num, unit)| match unit {
                b'h' => num * 3600,
                b'm' => num * 60,
                b's' => num,
                _ => 0,
            })
            .sum();
        Ok(VideoDuration(duration))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::VideoDuration;
    type Error = Box<dyn std::error::Error>;

    #[derive(Deserialize)]
    struct Holder {
        pub duration: VideoDuration,
    }

    #[test]
    fn parse_duration() -> Result<(), Error> {
        let holder: Holder = serde_json::from_str(r#"{"duration": "1h02m3s"}"#)?;
        assert_eq!(holder.duration.0, 3723);
        assert_eq!(holder.duration.to_string(), "01h02m03s");
        assert_eq!(VideoDuration(10).to_string(), "00h00m10s");
        Ok(())
    }
}
