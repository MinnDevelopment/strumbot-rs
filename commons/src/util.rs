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
