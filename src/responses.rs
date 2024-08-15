use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct AuthResponse {
    access_token: String,
    token_type: String,
}

impl AuthResponse {
    pub fn new(access_token: String, token_type: String) -> Self {
        Self {
            access_token,
            token_type,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MeResponse {
    pub email: String,
}

impl MeResponse {
    pub fn new(email: String) -> Self {
        Self { email }
    }
}

pub trait CursorDefault {
    fn id(&self) -> String;
}

#[derive(Debug, Clone, Serialize)]
pub struct PagedResponse<T> {
    data: Vec<T>,
    last: Option<String>,
}

impl<T: CursorDefault> PagedResponse<T> {
    pub fn new(data: Vec<T>) -> Self {
        let last = data.last().map(CursorDefault::id);
        Self { data, last }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UrlRedirect {
    id: Uuid,
    key: String,
    pub target: String,
}

impl CursorDefault for UrlRedirect {
    fn id(&self) -> String {
        self.key.clone()
    }
}

impl UrlRedirect {
    pub fn new(id: Uuid, key: String, target: String) -> Self {
        Self { id, key, target }
    }
}
