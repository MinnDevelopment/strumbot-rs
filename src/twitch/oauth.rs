use hashbrown::HashMap;
use std::{
    str::FromStr,
    time::{Duration, Instant},
};

use bytes::Bytes;
use log::{error, warn};
use reqwest::{Client as HttpClient, Method};
use serde::Deserialize;

use crate::error::RequestError;

const BASE_URL: &str = "https://api.twitch.tv/helix";

fn get_url(endpoint: &str) -> String {
    format!("{}/{}", BASE_URL, endpoint)
}

pub enum QueryParams {
    None,
    With(Vec<(String, String)>),
}

impl QueryParams {
    pub fn builder() -> QueryBuilder {
        QueryBuilder(vec![])
    }
}

pub struct QueryBuilder(Vec<(String, String)>);

impl QueryBuilder {
    pub fn param(mut self, key: &str, value: String) -> Self {
        self.0.push((key.to_string(), value));
        self
    }

    pub fn build(self) -> QueryParams {
        if self.0.is_empty() {
            QueryParams::None
        } else {
            QueryParams::With(self.0)
        }
    }
}

macro_rules! build_query {
    ($($key:expr => $value:expr),*) => {
        QueryParams::builder()
            $(.param($key, $value.to_string()))*
            .build()
    };
}

pub struct OauthClient {
    pub params: ClientParams,
    pub http: HttpClient,
}

impl OauthClient {
    const MAX_BACKOFF: Duration = Duration::from_secs(16);
    const MIN_BACKOFF: Duration = Duration::from_secs(1);

    pub fn new(params: ClientParams) -> Self {
        Self {
            params,
            http: HttpClient::new(),
        }
    }

    pub async fn authorize(&self) -> Result<Identity, RequestError> {
        let mut body = HashMap::with_capacity(3);
        body.insert("client_id", self.params.client_id.clone());
        body.insert("client_secret", self.params.client_secret.clone());
        body.insert("grant_type", "client_credentials".into());

        let endpoint = "https://id.twitch.tv/oauth2/token".to_string();

        let mut backoff = Self::MIN_BACKOFF;
        for _ in 0..10 {
            let response = self.http.post(&endpoint).form(&body).send().await;

            match response {
                Ok(res) if res.status().is_success() => {
                    return Ok(res.json::<Identity>().await?);
                }
                Ok(res) if res.status().is_server_error() => {
                    warn!("Server error: {}", res.status());
                }
                Ok(res) => {
                    return Err(RequestError::from(res.status()));
                }
                Err(err) if err.is_connect() => {
                    warn!("Connection error: {}", err);
                }
                Err(err) if err.is_timeout() => {
                    warn!("Request timeout: {}", err);
                }
                Err(err) if err.is_request() => {
                    warn!("Request error: {}", err);
                }
                Err(err) => {
                    error!("Request failed unexpectedly: {}", err);
                    return Err(RequestError::from(err));
                }
            };

            warn!("Retrying in {} seconds...", backoff.as_secs());
            tokio::time::sleep(backoff).await;
            backoff = Ord::clamp(backoff * 2, Self::MIN_BACKOFF, Self::MAX_BACKOFF);
        }

        Err(RequestError::Timeout)
    }

    /// Does not check if identity is expired, user error if so.
    async fn make_request<U, T, F>(
        &self,
        id: &Identity,
        method: Method,
        url: U,
        params: QueryParams,
        handler: F,
    ) -> Result<T, RequestError>
    where
        U: Into<String>,
        T: Sized + Send + Sync,
        F: FnOnce(Bytes) -> Result<T, RequestError>,
    {
        let mut full_url: String = url.into();

        if let QueryParams::With(vec) = params {
            let query = vec
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .reduce(|a, b| format!("{a}&{b}"));
            if let Some(query) = query {
                full_url.push('?');
                full_url.push_str(&query);
            }
        }

        let mut backoff = Self::MIN_BACKOFF;

        for _ in 0..10 {
            let request = self
                .http
                .request(method.clone(), full_url.clone())
                .header("Client-ID", self.params.client_id.as_ref())
                .bearer_auth(&id.access_token)
                .build()?;

            let response = self.http.execute(request).await;
            match response {
                Ok(res) if res.status().is_success() => {
                    return handler(res.bytes().await?);
                }
                Ok(res) if res.status().is_server_error() => {
                    warn!("Server error: {}", res.status());
                }
                Ok(res) if res.status().as_u16() == 429 => {
                    // skip standard exponential backoff for rate-limit retries since we already wait here
                    if let Some(header) = res.headers().get("Retry-After") {
                        match header.to_str()?.parse() {
                            Ok(retry_after) => {
                                warn!("Rate limit exceeded, retrying in {} seconds...", retry_after);
                                tokio::time::sleep(Duration::from_secs(retry_after)).await;
                                continue;
                            }
                            Err(err) => {
                                error!("Failed to parse Retry-After header {:?}: {}", header, err);
                            }
                        }
                    }
                    warn!("Rate limit exceeded, retrying in 10 seconds...");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
                Ok(res) => {
                    return Err(RequestError::from(res.status()));
                }
                Err(err) if err.is_connect() => {
                    warn!("Connection error: {}", err);
                }
                Err(err) if err.is_timeout() => {
                    warn!("Request timeout: {}", err);
                }
                Err(err) if err.is_request() => {
                    warn!("Request error: {}", err);
                }
                Err(err) => {
                    error!("Request failed unexpectedly: {}", err);
                    return Err(RequestError::from(err));
                }
            };

            warn!("Retrying in {} seconds...", backoff.as_secs());
            tokio::time::sleep(backoff).await;
            backoff = Ord::clamp(backoff * 2, Self::MIN_BACKOFF, Self::MAX_BACKOFF);
        }
        Err(RequestError::Timeout)
    }

    pub async fn get<F, T>(
        &self,
        id: &Identity,
        endpoint: &str,
        params: QueryParams,
        handler: F,
    ) -> Result<T, RequestError>
    where
        T: Sized + Send + Sync,
        F: FnOnce(Bytes) -> Result<T, RequestError>,
    {
        self.make_request(id, Method::GET, get_url(endpoint), params, handler)
            .await
    }
}

pub struct ClientParams {
    pub client_id: Box<str>,
    pub client_secret: Box<str>,
}

/// Client credentials identity according to https://dev.twitch.tv/docs/authentication/getting-tokens-oauth#client-credentials-grant-flow
#[derive(Deserialize, Clone)]
pub struct Identity {
    pub access_token: Box<str>,
    #[serde(with = "super::expires_at", rename = "expires_in")]
    pub expires_at: Instant,
    pub token_type: Box<str>,
}

impl FromStr for Identity {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_from_str() {
        let identity = Identity::from_str(
            r#"{
                "access_token": "jostpf5q0uzmxmkba9iyug38kjtgh",
                "expires_in": 5011271,
                "token_type": "bearer"
              }"#,
        )
        .unwrap();

        assert_eq!(identity.access_token.as_ref(), "jostpf5q0uzmxmkba9iyug38kjtgh");
        assert_eq!(identity.token_type.as_ref(), "bearer");
    }
}
