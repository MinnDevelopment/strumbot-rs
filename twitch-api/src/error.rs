use reqwest::{header::ToStrError, StatusCode};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RequestError {
    #[error("http request failed with code {0}")]
    Http(StatusCode),
    #[error("request timed out")]
    Timeout,
    #[error("unexpected error: {0:?}")]
    Unexpected(#[from] anyhow::Error),
    #[error("failed to deserialize {0:?}")]
    Deserialize(#[from] serde_json::Error),
    #[error("{0} not found for query {1}")]
    NotFound(&'static str, String),
}

impl From<reqwest::Error> for RequestError {
    fn from(e: reqwest::Error) -> Self {
        RequestError::Unexpected(e.into())
    }
}

impl From<ToStrError> for RequestError {
    fn from(e: ToStrError) -> Self {
        RequestError::Unexpected(e.into())
    }
}

impl From<StatusCode> for RequestError {
    fn from(code: StatusCode) -> Self {
        RequestError::Http(code)
    }
}
