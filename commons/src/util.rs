use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{num::NonZeroU64, ops::Add};

#[macro_export]
macro_rules! resolve {
    ($x:expr) => {
        match $x.await {
            Ok(response) => response.model().await.map_err(anyhow::Error::from),
            Err(err) => Err(anyhow::Error::from(err)),
        }
    };
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

// [0x1f600,0x1f603,0x1f604,...,0x1f1f3]
static EMOJI_LIST: [u32; 9280] = include!("../../emojis.txt");

static ALT_TEXT_WHITELIST: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*(?:[_*`]+|~~+|\|\|+)\s*|(\s+|^)\w+://").unwrap());

pub fn sanitize_link_title(text: &str) -> String {
    ALT_TEXT_WHITELIST
        .replace_all(text, " ")
        .chars()
        .filter(|c| !EMOJI_LIST.contains(&(*c as u32)))
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_markdown() {
        assert_eq!(
            sanitize_link_title("simple | text - with $ special & chars %"),
            "simple | text - with $ special & chars %"
        );
        assert_eq!(
            sanitize_link_title("hello world https://example.com"),
            "hello world example.com"
        );
        assert_eq!(
            sanitize_link_title("https://example.com hello world"),
            "example.com hello world"
        );
        assert_eq!(sanitize_link_title("~~hello world~~"), "hello world");
        assert_eq!(sanitize_link_title("*hello world*"), "hello world");
        assert_eq!(sanitize_link_title("hello ||world||"), "hello world");

        assert_eq!(
            sanitize_link_title("MORE LIKE SPLIT-LATE!!!! 😂😂😂"),
            "MORE LIKE SPLIT-LATE!!!!"
        )
    }
}
