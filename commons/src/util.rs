use fxhash::FxHashSet;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{num::NonZeroU64, ops::Add};

#[macro_export]
macro_rules! resolve {
    ($x:expr) => {
        match $x.await {
            Ok(response) => response.model().await.map_err(commons::errors::AsyncError::from),
            Err(err) => Err(commons::errors::AsyncError::from(err)),
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

const EMOJI_ARRAY: [u32; 9280] = include!("../../emojis.txt");

static EMOJI_SET: Lazy<FxHashSet<u32>> = Lazy::new(|| EMOJI_ARRAY.iter().copied().collect());

pub fn strip_emoji(text: &str) -> String {
    text.chars()
        .filter(|&c| !EMOJI_SET.contains(&(c as u32)))
        .fold(String::with_capacity(text.len()), |mut acc, c| {
            if c.is_whitespace() || c.is_control() {
                acc.push(' ');
            } else {
                acc.push(c);
            }
            acc
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_emoji() {
        assert_eq!(strip_emoji("no emoji"), "no emoji");
        assert_eq!(strip_emoji("simple smiley 😄"), "simple smiley ");
        assert_eq!(strip_emoji("with skin tone 👍🏽"), "with skin tone ");
        assert_eq!(strip_emoji("basic flag 🇩🇪 "), "basic flag  ");
        assert_eq!(
            strip_emoji("simple | text - with $ special & chars %"),
            "simple | text - with $ special & chars %"
        );
    }
}
