use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AuthRequest {
    pub authorization_code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListUrl {
    pub after: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedirectUrlPathParam {
    pub key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewUrl {
    pub key: String,
    pub target: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedirectUrlIdPathParam {
    pub id: uuid::Uuid,
}
