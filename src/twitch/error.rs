use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub enum TwitchError {
    UserNotFound(String),
}

impl Display for TwitchError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            TwitchError::UserNotFound(user) => write!(f, "User {} not found", user),
        }
    }
}

impl Error for TwitchError {}
