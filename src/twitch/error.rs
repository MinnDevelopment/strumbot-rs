use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub enum TwitchError {
    NotFound(String, String),
}

impl Display for TwitchError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            TwitchError::NotFound(kind, query) => write!(f, "{kind} not found for query {query}"),
        }
    }
}

impl Error for TwitchError {}
