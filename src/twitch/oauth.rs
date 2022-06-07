use std::{
    collections::HashMap,
    str::FromStr,
    time::{Duration, Instant},
};

use bytes::Bytes;
use log::warn;
use reqwest::{Client as HttpClient, Method};
use serde::Deserialize;

use super::{
    error::{AuthorizationError, HttpError, RequestTimeoutError},
    Error,
};

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
    pub fn new(params: ClientParams) -> Self {
        Self {
            params,
            http: HttpClient::new(),
        }
    }

    pub async fn authorize(&self) -> Result<Identity, AuthorizationError> {
        let mut body = HashMap::with_capacity(3);
        body.insert("client_id", self.params.client_id.clone());
        body.insert("client_secret", self.params.client_secret.clone());
        body.insert("grant_type", "client_credentials".to_string());

        let endpoint = "https://id.twitch.tv/oauth2/token".to_string();

        // TODO: Exponential backoff, proper handling for individual error codes
        loop {
            let response = self.http.post(&endpoint).form(&body).send().await;

            if let Ok(res) = response {
                if res.status().is_server_error() {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                } else if !res.status().is_success() {
                    return Err(AuthorizationError);
                }

                if let Ok(id) = res.json::<Identity>().await {
                    return Ok(id);
                }
            }

            // TODO: Handle error.is_timeout() and is_connect()

            return Err(AuthorizationError);
        }
    }

    /// Does not check if identity is expired, user error if so.
    async fn make_request<U, T, F>(
        &self,
        id: &Identity,
        method: Method,
        url: U,
        params: QueryParams,
        handler: F,
    ) -> Result<T, Error>
    where
        U: Into<String>,
        T: Sized + Send + Sync,
        F: FnOnce(Bytes) -> Result<T, Error>,
    {
        let mut full_url: String = url.into();

        if let QueryParams::With(vec) = params {
            let query = vec
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .reduce(|a, b| format!("{a}&{b}"))
                .unwrap();
            if !query.is_empty() {
                full_url.push('?');
                full_url.push_str(&query);
            }
        }

        // TODO: logging
        for _ in 0..3 {
            let request = self
                .http
                .request(method.clone(), full_url.clone())
                .header("Client-ID", &self.params.client_id)
                .bearer_auth(&id.access_token)
                .build()?;

            // TODO: handle timeout and connect errors
            let res = self.http.execute(request).await?;
            match res.status().as_u16() {
                x if x >= 500 => {
                    warn!("Server error {x}, retrying...");
                    continue;
                }
                401 => {
                    return Err(Box::new(AuthorizationError));
                }
                429 => {
                    if let Some(header) = res.headers().get("Retry-After") {
                        let retry_after = header.to_str()?.parse()?;
                        warn!(
                            "Rate limit exceeded, retrying in {} seconds...",
                            retry_after
                        );
                        tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    } else {
                        warn!("Rate limit exceeded, retrying in 10 seconds...");
                        tokio::time::sleep(Duration::from_secs(10)).await;
                    }
                    continue;
                }
                x if x < 300 => {
                    return handler(res.bytes().await?);
                }
                _ => {
                    return Err(Box::new(HttpError::from(res).await?));
                }
            }
        }
        Err(Box::new(RequestTimeoutError))
    }

    pub async fn get<F, T>(
        &self,
        id: &Identity,
        endpoint: &str,
        params: QueryParams,
        handler: F,
    ) -> Result<T, Error>
    where
        T: Sized + Send + Sync,
        F: FnOnce(Bytes) -> Result<T, Error>,
    {
        self.make_request(id, Method::GET, get_url(endpoint), params, handler)
            .await
    }
}

pub struct ClientParams {
    pub client_id: String,
    pub client_secret: String,
}

/// Client credentials identity according to https://dev.twitch.tv/docs/authentication/getting-tokens-oauth#client-credentials-grant-flow
#[derive(Deserialize, Clone)]
pub struct Identity {
    pub access_token: String,
    #[serde(with = "super::expires_at", rename = "expires_in")]
    pub expires_at: Instant,
    pub token_type: String,
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

        assert_eq!(identity.access_token, "jostpf5q0uzmxmkba9iyug38kjtgh");
        assert_eq!(identity.token_type, "bearer");
    }
}
