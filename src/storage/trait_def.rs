use crate::models::ShortenedUrl;
use anyhow::Result;
use async_trait::async_trait;

pub fn encode_base36_lower(id: i64) -> String {
    assert!(id >= 0, "identifier must be non-negative");
    let mut value = id as u64;
    let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_string();
    }
    let mut buf = Vec::new();
    while value > 0 {
        let rem = (value % 36) as usize;
        buf.push(digits[rem] as char);
        value /= 36;
    }
    buf.into_iter().rev().collect()
}

#[async_trait]
pub trait Storage: Send + Sync {
    /// Initialize the storage (run migrations, etc.)
    async fn init(&self) -> Result<()>;

    /// Create a new shortened URL with a caller-provided code (used for custom codes)
    async fn create_with_code(
        &self,
        short_code: &str,
        original_url: &str,
        created_by: Option<&str>,
    ) -> Result<ShortenedUrl>;

    /// Create a new shortened URL letting the storage derive the code from its identifier
    async fn create_auto(
        &self,
        original_url: &str,
        created_by: Option<&str>,
    ) -> Result<ShortenedUrl>;

    /// Get a shortened URL by short code
    async fn get(&self, short_code: &str) -> Result<Option<ShortenedUrl>>;

    /// Deactivate a shortened URL (soft delete)
    async fn deactivate(&self, short_code: &str) -> Result<bool>;

    /// Reactivate a shortened URL
    async fn reactivate(&self, short_code: &str) -> Result<bool>;

    /// Increment click count
    async fn increment_clicks(&self, short_code: &str) -> Result<()>;

    /// List all URLs (with pagination)
    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<ShortenedUrl>>;

    /// Check if short code exists
    async fn exists(&self, short_code: &str) -> Result<bool>;
}
