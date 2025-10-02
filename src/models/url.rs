use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ShortenedUrl {
    pub id: i64,
    pub short_code: String,
    pub original_url: String,
    pub created_at: i64,
    pub created_by: Option<String>,
    pub clicks: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUrlRequest {
    pub url: String,
    pub custom_code: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUrlRequest {
    pub url: Option<String>,
    pub expires_at: Option<i64>,
}
