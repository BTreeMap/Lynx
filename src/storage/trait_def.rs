use crate::models::ShortenedUrl;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("short code already exists")]
    Conflict,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Metadata about a storage lookup operation
#[derive(Debug, Clone)]
pub struct LookupMetadata {
    /// Whether the result was served from cache
    pub cache_hit: bool,
    /// Time spent in cache lookup (if cache hit)
    pub cache_duration: Option<Duration>,
    /// Time spent in database lookup (if cache miss)
    pub db_duration: Option<Duration>,
}

/// Result of a storage lookup with metadata
#[derive(Debug, Clone)]
pub struct LookupResult {
    /// The URL data, if found
    pub url: Option<Arc<ShortenedUrl>>,
    /// Metadata about the lookup operation
    pub metadata: LookupMetadata,
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
    ) -> StorageResult<Arc<ShortenedUrl>>;

    // Additional helper methods may be added for automatic code generation if storage-backed.

    /// Get a shortened URL by short code with metadata (cache hit/miss, timing info)
    async fn get(&self, short_code: &str) -> Result<LookupResult>;

    /// Get a shortened URL by short code with authoritative statistics
    async fn get_authoritative(&self, short_code: &str) -> Result<Option<Arc<ShortenedUrl>>>;

    /// Deactivate a shortened URL (soft delete)
    async fn deactivate(&self, short_code: &str) -> Result<bool>;

    /// Reactivate a shortened URL
    async fn reactivate(&self, short_code: &str) -> Result<bool>;

    /// Increment click count by the provided amount
    async fn increment_clicks(&self, short_code: &str, amount: u64) -> Result<()>;

    /// Increment click count by 1 (convenience helper)
    async fn increment_click(&self, short_code: &str) -> Result<()> {
        self.increment_clicks(short_code, 1).await
    }

    /// List URLs with cursor-based pagination
    /// Returns URLs ordered by created_at DESC, id DESC
    /// If cursor is provided, returns URLs created before that cursor position
    /// Returns up to limit results (caller should request limit+1 to determine if there are more pages)
    async fn list_with_cursor(
        &self,
        limit: i64,
        cursor: Option<(i64, i64)>, // (created_at, id)
        is_admin: bool,
        user_id: Option<&str>,
    ) -> Result<Vec<Arc<ShortenedUrl>>>;

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

    /// Patch created_by for a specific short code
    async fn patch_created_by(&self, short_code: &str, new_created_by: &str) -> Result<bool>;

    /// Patch all malformed created_by values (all-zero UUID or null) to a new value
    /// Returns the number of rows updated
    async fn patch_all_malformed_created_by(&self, new_created_by: &str) -> Result<i64>;

    /// List all users with pagination support
    /// Returns users ordered by created_at DESC
    /// Returns up to limit results
    async fn list_all_users(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(String, String, String, i64)>>; // (user_id, auth_method, email, created_at)

    /// List all links created by a specific user with pagination
    /// Returns links ordered by created_at DESC
    async fn list_user_links(
        &self,
        user_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Arc<ShortenedUrl>>>;

    /// Deactivate all links created by a specific user
    /// Returns the number of links deactivated
    async fn bulk_deactivate_user_links(&self, user_id: &str) -> Result<i64>;

    /// Reactivate all links created by a specific user
    /// Returns the number of links reactivated
    async fn bulk_reactivate_user_links(&self, user_id: &str) -> Result<i64>;

    /// Batch insert or update analytics records
    /// Uses UPSERT to increment visit counts for existing records
    async fn upsert_analytics_batch(
        &self,
        records: Vec<(String, i64, Option<String>, Option<String>, Option<String>, Option<i64>, i32, i64)>,
    ) -> Result<()>;
    // (short_code, time_bucket, country_code, region, city, asn, ip_version, count)

    /// Get analytics for a specific short code
    async fn get_analytics(
        &self,
        short_code: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: i64,
    ) -> Result<Vec<crate::analytics::AnalyticsEntry>>;

    /// Get aggregated analytics grouped by a dimension
    async fn get_analytics_aggregate(
        &self,
        short_code: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        group_by: &str,
        limit: i64,
    ) -> Result<Vec<crate::analytics::AnalyticsAggregate>>;

    /// Prune old analytics data by aggregating entries and dropping specified dimensions
    /// Returns the number of rows affected (deleted old rows + inserted aggregated rows)
    async fn prune_analytics(
        &self,
        retention_days: i64,
        drop_dimensions: &[String],
    ) -> Result<(i64, i64)>; // (deleted_count, inserted_count)

    /// Get the difference between total clicks and analytics visit count for a short code
    /// Returns (clicks, analytics_count, difference)
    async fn get_analytics_click_difference(
        &self,
        short_code: &str,
    ) -> Result<(i64, i64, i64)>;

    /// Insert alignment entry to reconcile analytics with click count
    /// Returns the number of rows inserted
    async fn align_analytics_with_clicks(
        &self,
        short_code: &str,
    ) -> Result<i64>;
}
