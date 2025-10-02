use crate::models::ShortenedUrl;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Storage: Send + Sync {
    /// Initialize the storage (run migrations, etc.)
    async fn init(&self) -> Result<()>;
    
    /// Create a new shortened URL
    async fn create(&self, short_code: &str, original_url: &str, created_by: Option<&str>, expires_at: Option<i64>) -> Result<ShortenedUrl>;
    
    /// Get a shortened URL by short code
    async fn get(&self, short_code: &str) -> Result<Option<ShortenedUrl>>;
    
    /// Update a shortened URL
    async fn update(&self, short_code: &str, original_url: Option<&str>, expires_at: Option<i64>) -> Result<bool>;
    
    /// Delete a shortened URL
    async fn delete(&self, short_code: &str) -> Result<bool>;
    
    /// Increment click count
    async fn increment_clicks(&self, short_code: &str) -> Result<()>;
    
    /// List all URLs (with pagination)
    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<ShortenedUrl>>;
    
    /// Check if short code exists
    async fn exists(&self, short_code: &str) -> Result<bool>;
}
