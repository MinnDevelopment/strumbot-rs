use std::{
    error::Error,
    fmt::{Display, Formatter},
};

use reqwest::Response;

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

#[derive(Debug)]
pub struct HttpError {
    pub status: u16,
    pub body: String,
}

impl HttpError {
    pub fn new(status: u16, body: String) -> Self {
        Self { status, body }
    }

    pub async fn from(response: Response) -> Result<Self, reqwest::Error> {
        let code = response.status().as_u16();
        let body = response.text().await?;
        Ok(Self::new(code, body))
    }
}

impl Display for HttpError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "HTTP error {}: {}", self.status, self.body)
    }
}

impl Error for HttpError {}

#[derive(Debug)]
pub struct RequestTimeoutError;

impl Display for RequestTimeoutError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Request timeout after 3 tries")
    }
}

impl Error for RequestTimeoutError {}

#[derive(Debug)]
pub struct AuthorizationError;

impl Display for AuthorizationError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Authorization error")
    }
}

impl std::error::Error for AuthorizationError {}
