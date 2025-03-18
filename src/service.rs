use std::ops::Deref;

use axum::response::{IntoResponse, Response};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, ModelTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

use crate::{models::url_redirects, responses::UrlRedirect};

#[derive(Debug, thiserror::Error)]
pub enum InsertError {
    #[error("database error: {0}")]
    Database(sea_orm::DbErr),
    #[error("already exists")]
    KeyAlreadyExists,
}

impl From<sea_orm::DbErr> for InsertError {
    fn from(error: sea_orm::DbErr) -> Self {
        match error.sql_err() {
            Some(sea_orm::SqlErr::UniqueConstraintViolation(key))
                if key.contains("url_redirects_key_key") =>
            {
                Self::KeyAlreadyExists
            }
            _ => Self::Database(error),
        }
    }
}

impl From<InsertError> for Response {
    fn from(value: InsertError) -> Self {
        match value {
            InsertError::Database(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error",
            ),
            InsertError::KeyAlreadyExists => (http::StatusCode::CONFLICT, "key already exists"),
        }
        .into_response()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

impl From<QueryError> for Response {
    fn from(value: QueryError) -> Self {
        tracing::error!(error = %value, "service internal server error");
        (
            http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error",
        )
            .into_response()
    }
}

pub enum RedirectKeyValidationFailed {
    TooLong,
    InvalidCharacters(Vec<char>),
}

impl From<RedirectKeyValidationFailed> for Response {
    fn from(value: RedirectKeyValidationFailed) -> Self {
        match value {
            RedirectKeyValidationFailed::TooLong => (
                http::StatusCode::BAD_REQUEST,
                "too long, maximum length of a key is 100",
            )
                .into_response(),
            RedirectKeyValidationFailed::InvalidCharacters(chars) => {
                let invalid_chars: String = chars.into_iter().collect();
                (
                    http::StatusCode::BAD_REQUEST,
                    format!("invalid characters: {}", invalid_chars),
                )
                    .into_response()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RedirectKey(String);

impl Deref for RedirectKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<String> for RedirectKey {
    type Error = RedirectKeyValidationFailed;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() > 100 {
            return Err(RedirectKeyValidationFailed::TooLong);
        }

        let invalid_chars: Vec<char> = value
            .chars()
            .filter(|c| !c.is_ascii_alphanumeric() && *c != '-' && *c != '_')
            .collect();

        if !invalid_chars.is_empty() {
            return Err(RedirectKeyValidationFailed::InvalidCharacters(
                invalid_chars,
            ));
        }

        Ok(Self(value))
    }
}

#[derive(Debug, Clone)]
pub struct NewUrlRedirect {
    user_email: String,
    key: RedirectKey,
    target: String,
}

impl NewUrlRedirect {
    pub fn new(user_email: String, key: RedirectKey, target: String) -> Self {
        Self {
            user_email,
            key,
            target,
        }
    }
}

impl From<NewUrlRedirect> for url_redirects::ActiveModel {
    fn from(value: NewUrlRedirect) -> Self {
        url_redirects::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            user_email: Set(value.user_email),
            key: Set(value.key.0),
            target: Set(value.target),
            ..Default::default()
        }
    }
}

pub struct UrlService {
    db: DatabaseConnection,
}

impl UrlService {
    pub async fn new(postgres_url: &str) -> Result<Self, DbErr> {
        Ok(Self {
            db: sea_orm::Database::connect(postgres_url).await?,
        })
    }
}

impl UrlService {
    pub async fn list_by_email(
        &self,
        user_email: &str,
        after: Option<String>,
        limit: u64,
    ) -> Result<Vec<UrlRedirect>, QueryError> {
        let mut query = url_redirects::Entity::find()
            .filter(url_redirects::Column::UserEmail.eq(user_email))
            .order_by_asc(url_redirects::Column::Key)
            .limit(limit);

        if let Some(key) = after {
            query = query.filter(url_redirects::Column::Key.gt(key));
        }

        Ok(query
            .all(&self.db)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn get_by_id_and_email(
        &self,
        id: uuid::Uuid,
        email: &str,
    ) -> Result<Option<UrlRedirect>, QueryError> {
        Ok(url_redirects::Entity::find()
            .filter(url_redirects::Column::Id.eq(id))
            .filter(url_redirects::Column::UserEmail.eq(email))
            .one(&self.db)
            .await?
            .map(Into::into))
    }

    pub async fn get_by_key(&self, key: &str) -> Result<Option<UrlRedirect>, QueryError> {
        Ok(url_redirects::Entity::find()
            .filter(url_redirects::Column::Key.eq(key))
            .one(&self.db)
            .await?
            .map(Into::into))
    }

    pub async fn create(&self, new_url: NewUrlRedirect) -> Result<UrlRedirect, InsertError> {
        url_redirects::ActiveModel::from(new_url)
            .insert(&self.db)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn delete(
        &self,
        user_email: &str,
        id: uuid::Uuid,
    ) -> Result<Option<UrlRedirect>, QueryError> {
        let url = url_redirects::Entity::find_by_id(id)
            .filter(url_redirects::Column::UserEmail.eq(user_email))
            .one(&self.db)
            .await?;

        let Some(url) = url else { return Ok(None) };

        url.clone().delete(&self.db).await?;
        Ok(Some(url.into()))
    }

    pub async fn update(
        &self,
        id: uuid::Uuid,
        new_url: NewUrlRedirect,
    ) -> Result<Option<UrlRedirect>, InsertError> {
        let url = url_redirects::Entity::find_by_id(id)
            .filter(url_redirects::Column::UserEmail.eq(new_url.user_email))
            .one(&self.db)
            .await?;

        let Some(url) = url else { return Ok(None) };

        let mut active_model = url_redirects::ActiveModel::from(url);
        active_model.key = Set(new_url.key.0);
        active_model.target = Set(new_url.target);
        active_model.updated_at = Set(chrono::Utc::now().into());

        let url = active_model.update(&self.db).await?;
        Ok(Some(url.into()))
    }
}

impl From<url_redirects::Model> for UrlRedirect {
    fn from(value: url_redirects::Model) -> Self {
        Self::new(value.id, value.key, value.target)
    }
}
