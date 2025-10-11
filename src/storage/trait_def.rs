use crate::models::ShortenedUrl;
use anyhow::Result;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("short code already exists")]
    Conflict,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type StorageResult<T> = Result<T, StorageError>;

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
    ) -> StorageResult<ShortenedUrl>;

    // Additional helper methods may be added for automatic code generation if storage-backed.

    /// Get a shortened URL by short code
    async fn get(&self, short_code: &str) -> Result<Option<ShortenedUrl>>;

    /// Deactivate a shortened URL (soft delete)
    async fn deactivate(&self, short_code: &str) -> Result<bool>;

    /// Reactivate a shortened URL
    async fn reactivate(&self, short_code: &str) -> Result<bool>;

    /// Increment click count
    async fn increment_clicks(&self, short_code: &str) -> Result<()>;

    /// List all URLs (with pagination and optional user filtering)
    /// If is_admin is true, returns all URLs regardless of user_id
    /// Otherwise, returns only URLs created by the specified user_id
    async fn list(
        &self,
        limit: i64,
        offset: i64,
        is_admin: bool,
        user_id: Option<&str>,
    ) -> Result<Vec<ShortenedUrl>>;

    /// Register or update user metadata
    async fn upsert_user(
        &self,
        user_id: &str,
        email: Option<&str>,
        auth_method: &str,
    ) -> Result<()>;

    /// Check if a user is manually promoted to admin
    async fn is_manual_admin(&self, user_id: &str, auth_method: &str) -> Result<bool>;

    /// Promote a user to admin manually
    async fn promote_to_admin(&self, user_id: &str, auth_method: &str) -> Result<()>;

    /// Demote a user from admin
    async fn demote_from_admin(&self, user_id: &str, auth_method: &str) -> Result<bool>;

    /// List all manually promoted admins
    async fn list_manual_admins(&self) -> Result<Vec<(String, String, String)>>; // (user_id, auth_method, email)
}
