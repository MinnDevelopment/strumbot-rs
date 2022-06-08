use std::{
    error::Error,
    fmt::{Display, Formatter},
};

use reqwest::{header::ToStrError, StatusCode};

pub type AsyncError = Box<dyn Error + Send + Sync>;
pub type SerdeError = serde_json::Error;

#[derive(Debug)]
pub enum RequestError {
    Http(StatusCode),
    Timeout,
    Unexpected(AsyncError),
    Deserialize(SerdeError),
    NotFound(String, String),
}

impl Display for RequestError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            RequestError::Http(status) => write!(f, "HTTP {}", status),
            RequestError::Unexpected(e) => write!(f, "Unexpected error: {}", e),
            RequestError::Deserialize(e) => write!(f, "Deserialize error: {}", e),
            RequestError::Timeout => write!(f, "Timeout error"),
            RequestError::NotFound(kind, query) => write!(f, "{kind} not found for query {query}"),
        }
    }
}

impl std::error::Error for RequestError {}

impl From<reqwest::Error> for RequestError {
    fn from(e: reqwest::Error) -> Self {
        RequestError::Unexpected(Box::new(e))
    }
}

impl From<AsyncError> for RequestError {
    fn from(e: AsyncError) -> Self {
        RequestError::Unexpected(e)
    }
}

impl From<ToStrError> for RequestError {
    fn from(e: ToStrError) -> Self {
        RequestError::Unexpected(Box::new(e))
    }
}

impl From<StatusCode> for RequestError {
    fn from(code: StatusCode) -> Self {
        RequestError::Http(code)
    }
}

impl From<SerdeError> for RequestError {
    fn from(e: SerdeError) -> Self {
        RequestError::Deserialize(e)
    }
}
