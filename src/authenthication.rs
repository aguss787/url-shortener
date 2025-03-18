use std::sync::Arc;

use axum::{
    async_trait,
    extract::FromRequestParts,
    response::{IntoResponse, Response},
};
use http::StatusCode;
use redis::{AsyncCommands, SetOptions};

use crate::{
    kvs::{KvsError, KvsPool, KvsPoolError},
    responses::AuthResponse,
    Services,
};

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("internal error: {0}")]
    Internal(Box<dyn std::error::Error>),
}

impl From<KvsPoolError> for AuthenticationError {
    fn from(error: KvsPoolError) -> Self {
        Self::Internal(Box::new(error))
    }
}

impl From<KvsError> for AuthenticationError {
    fn from(error: KvsError) -> Self {
        Self::Internal(Box::new(error))
    }
}

impl IntoResponse for AuthenticationError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Internal(error) => {
                tracing::error!(%error, "internal server error on authentication");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
            }
        }
        .into_response()
    }
}

impl From<AuthenticationError> for Response {
    fn from(value: AuthenticationError) -> Self {
        value.into_response()
    }
}

#[derive(Debug, Clone)]
pub struct Requester {
    pub email: String,
}

#[async_trait]
impl FromRequestParts<Arc<Services>> for Requester {
    type Rejection = AuthenticationError;

    async fn from_request_parts(
        parts: &mut http::request::Parts,
        state: &Arc<Services>,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(http::header::AUTHORIZATION)
            .ok_or(AuthenticationError::Unauthorized)?
            .to_str()
            .map_err(|_| AuthenticationError::Unauthorized)?;

        let email = state.auth.introspect_token(header).await?;

        Ok(Self { email })
    }
}

pub struct AuthenticationService {
    host: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    kvs_pool: Arc<KvsPool>,
}

impl AuthenticationService {
    pub fn new(
        host: String,
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        kvs_pool: Arc<KvsPool>,
    ) -> Self {
        Self {
            host,
            client_id,
            client_secret,
            redirect_uri,
            kvs_pool,
        }
    }

    async fn introspect_token(&self, header: &str) -> Result<String, AuthenticationError> {
        if let Ok(Some(email)) = self
            .get_cached_token(header)
            .await
            .inspect_err(|error| tracing::error!(%error, "failed to get token from cache"))
        {
            tracing::debug!(email, "cache found, skipping profile call");
            return Ok(email);
        }

        let client = reqwest::Client::new();
        let result = client
            .get(format!("{}/profile", self.host))
            .header(http::header::AUTHORIZATION, header)
            .send()
            .await
            .map_err(|error| AuthenticationError::Internal(Box::new(error)))?;

        #[derive(Debug, serde::Deserialize)]
        struct Profile {
            email: String,
        }

        let response = match result.status() {
            StatusCode::UNAUTHORIZED => Err(AuthenticationError::Unauthorized),
            StatusCode::BAD_REQUEST => Err(AuthenticationError::Unauthorized),
            StatusCode::OK => result
                .json::<Profile>()
                .await
                .map_err(|error| AuthenticationError::Internal(Box::new(error))),
            _ => {
                tracing::error!("unexpected status code: {:?}", result.status());

                Err(AuthenticationError::Internal(Box::new(
                    std::io::Error::other("unexpected status code"),
                )))
            }
        }?;

        // cache the token in the background.
        // if it fails, just log the error and continue.
        let email = response.email.clone();
        let token = header.to_string();
        let kvs_pool = self.kvs_pool.clone();
        tokio::spawn(async move {
            cache_token(kvs_pool, &token, &email)
                .await
                .inspect_err(|error| {
                    tracing::error!(%error, "failed to store token cache");
                })
                .ok();
        });

        Ok(response.email)
    }

    pub async fn exchange_token(
        &self,
        authorization_code: &str,
    ) -> Result<AuthResponse, AuthenticationError> {
        let client = reqwest::Client::new();

        #[derive(Debug, serde::Serialize)]
        struct TokenRequest<'a> {
            grant_type: &'a str,
            client_id: &'a str,
            client_secret: &'a str,
            redirect_uri: &'a str,
            code: &'a str,
        }
        let result = client
            .post(format!("{}/oauth2/token", self.host))
            .form(&TokenRequest {
                grant_type: "authorization_code",
                client_id: &self.client_id,
                client_secret: &self.client_secret,
                redirect_uri: &self.redirect_uri,
                code: authorization_code,
            })
            .send()
            .await
            .map_err(|error| AuthenticationError::Internal(Box::new(error)))?;

        #[derive(Debug, serde::Deserialize)]
        struct TokenResponse {
            access_token: String,
            token_type: String,
        }
        let response = match result.status() {
            StatusCode::BAD_REQUEST => Err(AuthenticationError::Unauthorized),
            StatusCode::OK => result
                .json::<TokenResponse>()
                .await
                .map_err(|error| AuthenticationError::Internal(Box::new(error))),
            _ => {
                tracing::error!("unexpected status code: {:?}", result.status());

                Err(AuthenticationError::Internal(Box::new(
                    std::io::Error::other("unexpected status code"),
                )))
            }
        }?;

        Ok(AuthResponse::new(
            response.access_token,
            response.token_type,
        ))
    }
}

// Code below is for caching the token

impl AuthenticationService {
    #[tracing::instrument(skip(self, token))]
    async fn get_cached_token(&self, token: &str) -> Result<Option<String>, AuthenticationError> {
        let mut conn = self.kvs_pool.get().await?;
        let key = token_key(token);

        conn.get(key).await.map_err(Into::into)
    }
}

#[tracing::instrument(skip(kvs_pool, token, value))]
async fn cache_token(
    kvs_pool: Arc<KvsPool>,
    token: &str,
    value: &str,
) -> Result<(), AuthenticationError> {
    let mut conn = kvs_pool.get().await?;
    let key = token_key(token);

    conn.set_options(
        key,
        value,
        SetOptions::default()
            .conditional_set(redis::ExistenceCheck::NX)
            .with_expiration(redis::SetExpiry::EX(30)),
    )
    .await
    .map_err(Into::into)
}

fn token_key(token: &str) -> String {
    format!("token:{token}")
}
