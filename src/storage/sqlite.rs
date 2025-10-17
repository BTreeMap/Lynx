use crate::models::ShortenedUrl;
use crate::storage::{LookupMetadata, LookupResult, Storage, StorageError, StorageResult};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::convert::TryFrom;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

pub struct SqliteStorage {
    pool: Arc<SqlitePool>,
}

impl SqliteStorage {
    pub async fn new(database_url: &str, max_connections: u32) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

        let db_path = options.get_filename();

        if db_path != Path::new(":memory:") {
            if let Some(parent) = db_path.parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent).await?;
                }
            }
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect_with(options)
            .await?;
        Ok(Self {
            pool: Arc::new(pool),
        })
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn init(&self) -> Result<()> {
        // Create URLs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS urls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                short_code TEXT NOT NULL UNIQUE,
                original_url TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                created_by TEXT,
                clicks INTEGER NOT NULL DEFAULT 0,
                is_active INTEGER NOT NULL DEFAULT 1
            )
            "#,
        )
        .execute(self.pool.as_ref())
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_short_code ON urls(short_code)")
            .execute(self.pool.as_ref())
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_created_by ON urls(created_by)")
            .execute(self.pool.as_ref())
            .await?;

        // Index for cursor-based pagination (created_at DESC, id DESC)
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_urls_created_at_id ON urls(created_at DESC, id DESC)",
        )
        .execute(self.pool.as_ref())
        .await?;

        // Index for user-specific cursor pagination
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_urls_created_by_created_at_id ON urls(created_by, created_at DESC, id DESC)",
        )
        .execute(self.pool.as_ref())
        .await?;

        // Create users table to track user metadata
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                user_id TEXT NOT NULL,
                auth_method TEXT NOT NULL,
                email TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (user_id, auth_method)
            )
            "#,
        )
        .execute(self.pool.as_ref())
        .await?;

        // Create admin_users table for manually promoted admins
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS admin_users (
                user_id TEXT NOT NULL,
                auth_method TEXT NOT NULL,
                promoted_at INTEGER NOT NULL,
                PRIMARY KEY (user_id, auth_method)
            )
            "#,
        )
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn create_with_code(
        &self,
        short_code: &str,
        original_url: &str,
        created_by: Option<&str>,
    ) -> StorageResult<Arc<ShortenedUrl>> {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| StorageError::Other(e.into()))?
            .as_secs() as i64;

        let result = sqlx::query(
            r#"
            INSERT INTO urls (short_code, original_url, created_at, created_by, is_active)
            VALUES (?, ?, ?, ?, 1)
            ON CONFLICT(short_code) DO NOTHING
            "#,
        )
        .bind(short_code)
        .bind(original_url)
        .bind(created_at)
        .bind(created_by)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| StorageError::Other(e.into()))?;

        if result.rows_affected() == 0 {
            return Err(StorageError::Conflict);
        }

        let url = sqlx::query_as::<_, ShortenedUrl>(
            r#"
            SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
            FROM urls
            WHERE short_code = ?
            "#,
        )
        .bind(short_code)
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(|e| StorageError::Other(e.into()))?;

        Ok(Arc::new(url))
    }

    async fn get(&self, short_code: &str) -> Result<LookupResult> {
        let start = Instant::now();
        let url = self.get_authoritative(short_code).await?;
        let duration = start.elapsed();

        Ok(LookupResult {
            url,
            metadata: LookupMetadata {
                cache_hit: false,
                cache_duration: None,
                db_duration: Some(duration),
            },
        })
    }

    async fn get_authoritative(&self, short_code: &str) -> Result<Option<Arc<ShortenedUrl>>> {
        let url = sqlx::query_as::<_, ShortenedUrl>(
            r#"
            SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
            FROM urls
            WHERE short_code = ?
            "#,
        )
        .bind(short_code)
        .fetch_optional(self.pool.as_ref())
        .await?;

        Ok(url.map(Arc::new))
    }

    async fn deactivate(&self, short_code: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE urls
            SET is_active = 0
            WHERE short_code = ?
            "#,
        )
        .bind(short_code)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn reactivate(&self, short_code: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE urls
            SET is_active = 1
            WHERE short_code = ?
            "#,
        )
        .bind(short_code)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn increment_clicks(&self, short_code: &str, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }

        let amount = i64::try_from(amount).map_err(|_| anyhow!("increment amount exceeds i64"))?;

        sqlx::query(
            r#"
            UPDATE urls
            SET clicks = clicks + ?
            WHERE short_code = ?
            "#,
        )
        .bind(amount)
        .bind(short_code)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn list_with_cursor(
        &self,
        limit: i64,
        cursor: Option<(i64, i64)>,
        is_admin: bool,
        user_id: Option<&str>,
    ) -> Result<Vec<Arc<ShortenedUrl>>> {
        let urls = if is_admin || user_id.is_none() {
            // Admin sees all URLs, or when auth is disabled (no user_id), show all
            if let Some((cursor_created_at, cursor_id)) = cursor {
                sqlx::query_as::<_, ShortenedUrl>(
                    r#"
                    SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
                    FROM urls
                    WHERE (created_at < ?) OR (created_at = ? AND id < ?)
                    ORDER BY created_at DESC, id DESC
                    LIMIT ?
                    "#,
                )
                .bind(cursor_created_at)
                .bind(cursor_created_at)
                .bind(cursor_id)
                .bind(limit)
                .fetch_all(self.pool.as_ref())
                .await?
            } else {
                sqlx::query_as::<_, ShortenedUrl>(
                    r#"
                    SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
                    FROM urls
                    ORDER BY created_at DESC, id DESC
                    LIMIT ?
                    "#,
                )
                .bind(limit)
                .fetch_all(self.pool.as_ref())
                .await?
            }
        } else if let Some(uid) = user_id {
            // Regular user sees only their own URLs
            if let Some((cursor_created_at, cursor_id)) = cursor {
                sqlx::query_as::<_, ShortenedUrl>(
                    r#"
                    SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
                    FROM urls
                    WHERE created_by = ? AND ((created_at < ?) OR (created_at = ? AND id < ?))
                    ORDER BY created_at DESC, id DESC
                    LIMIT ?
                    "#,
                )
                .bind(uid)
                .bind(cursor_created_at)
                .bind(cursor_created_at)
                .bind(cursor_id)
                .bind(limit)
                .fetch_all(self.pool.as_ref())
                .await?
            } else {
                sqlx::query_as::<_, ShortenedUrl>(
                    r#"
                    SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
                    FROM urls
                    WHERE created_by = ?
                    ORDER BY created_at DESC, id DESC
                    LIMIT ?
                    "#,
                )
                .bind(uid)
                .bind(limit)
                .fetch_all(self.pool.as_ref())
                .await?
            }
        } else {
            // Should not reach here, but return empty list
            vec![]
        };

        Ok(urls.into_iter().map(Arc::new).collect())
    }

    async fn upsert_user(
        &self,
        user_id: &str,
        email: Option<&str>,
        auth_method: &str,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        sqlx::query(
            r#"
            INSERT INTO users (user_id, auth_method, email, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT (user_id, auth_method) DO UPDATE SET
                email = COALESCE(excluded.email, users.email),
                updated_at = excluded.updated_at
            "#,
        )
        .bind(user_id)
        .bind(auth_method)
        .bind(email)
        .bind(now)
        .bind(now)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn is_manual_admin(&self, user_id: &str, auth_method: &str) -> Result<bool> {
        let result = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM admin_users
            WHERE user_id = ? AND auth_method = ?
            "#,
        )
        .bind(user_id)
        .bind(auth_method)
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(result > 0)
    }

    async fn promote_to_admin(&self, user_id: &str, auth_method: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        sqlx::query(
            r#"
            INSERT INTO admin_users (user_id, auth_method, promoted_at)
            VALUES (?, ?, ?)
            ON CONFLICT (user_id, auth_method) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(auth_method)
        .bind(now)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn demote_from_admin(&self, user_id: &str, auth_method: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM admin_users
            WHERE user_id = ? AND auth_method = ?
            "#,
        )
        .bind(user_id)
        .bind(auth_method)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn list_manual_admins(&self) -> Result<Vec<(String, String, String)>> {
        let admins = sqlx::query_as::<_, (String, String, Option<String>)>(
            r#"
            SELECT a.user_id, a.auth_method, u.email
            FROM admin_users a
            LEFT JOIN users u ON a.user_id = u.user_id AND a.auth_method = u.auth_method
            ORDER BY a.promoted_at DESC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await?
        .into_iter()
        .map(|(user_id, auth_method, email)| {
            (
                user_id,
                auth_method,
                email.unwrap_or_else(|| "N/A".to_string()),
            )
        })
        .collect();

        Ok(admins)
    }

    async fn patch_created_by(&self, short_code: &str, new_created_by: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE urls
            SET created_by = ?
            WHERE short_code = ?
            "#,
        )
        .bind(new_created_by)
        .bind(short_code)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn patch_all_malformed_created_by(&self, new_created_by: &str) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE urls
            SET created_by = ?
            WHERE created_by IS NULL 
               OR created_by = '' 
               OR created_by = '00000000-0000-0000-0000-000000000000'
            "#,
        )
        .bind(new_created_by)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() as i64)
    }

    async fn list_all_users(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(String, String, String, i64)>> {
        let users = sqlx::query_as::<_, (String, String, Option<String>, i64)>(
            r#"
            SELECT user_id, auth_method, email, created_at
            FROM users
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.as_ref())
        .await?
        .into_iter()
        .map(|(user_id, auth_method, email, created_at)| {
            (
                user_id,
                auth_method,
                email.unwrap_or_else(|| "N/A".to_string()),
                created_at,
            )
        })
        .collect();

        Ok(users)
    }

    async fn list_user_links(
        &self,
        user_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Arc<ShortenedUrl>>> {
        let urls = sqlx::query_as::<_, ShortenedUrl>(
            r#"
            SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
            FROM urls
            WHERE created_by = ?
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(urls.into_iter().map(Arc::new).collect())
    }

    async fn bulk_deactivate_user_links(&self, user_id: &str) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE urls
            SET is_active = 0
            WHERE created_by = ? AND is_active = 1
            "#,
        )
        .bind(user_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() as i64)
    }

    async fn bulk_reactivate_user_links(&self, user_id: &str) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE urls
            SET is_active = 1
            WHERE created_by = ? AND is_active = 0
            "#,
        )
        .bind(user_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_sqlite() -> Arc<dyn Storage> {
        let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
        storage.init().await.unwrap();
        Arc::new(storage)
    }

    async fn create_test_urls(storage: &Arc<dyn Storage>) {
        // Create URL with normal user
        storage
            .create_with_code("normal1", "https://example.com/1", Some("user123"))
            .await
            .unwrap();

        // Create URL with all-zero UUID (malformed)
        storage
            .create_with_code(
                "malformed1",
                "https://example.com/2",
                Some("00000000-0000-0000-0000-000000000000"),
            )
            .await
            .unwrap();

        // Create URL with empty string (malformed)
        storage
            .create_with_code("malformed2", "https://example.com/3", Some(""))
            .await
            .unwrap();

        // Create URL with null created_by (malformed)
        storage
            .create_with_code("malformed3", "https://example.com/4", None)
            .await
            .unwrap();

        // Create another normal URL
        storage
            .create_with_code("normal2", "https://example.com/5", Some("user456"))
            .await
            .unwrap();

        // Create another all-zero UUID URL
        storage
            .create_with_code(
                "malformed4",
                "https://example.com/6",
                Some("00000000-0000-0000-0000-000000000000"),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_patch_created_by_single_url() {
        let storage = setup_sqlite().await;
        create_test_urls(&storage).await;

        // Patch a single URL
        let updated = storage
            .patch_created_by("malformed1", "newuser789")
            .await
            .unwrap();
        assert!(updated, "Should have updated the URL");

        // Verify the patch
        let url = storage.get_authoritative("malformed1").await.unwrap();
        assert!(url.is_some());
        assert_eq!(url.unwrap().created_by, Some("newuser789".to_string()));

        // Verify other URLs are unchanged
        let url2 = storage.get_authoritative("normal1").await.unwrap();
        assert_eq!(url2.unwrap().created_by, Some("user123".to_string()));
    }

    #[tokio::test]
    async fn test_patch_created_by_nonexistent_url() {
        let storage = setup_sqlite().await;

        // Try to patch a URL that doesn't exist
        let updated = storage
            .patch_created_by("nonexistent", "newuser789")
            .await
            .unwrap();
        assert!(!updated, "Should not have updated nonexistent URL");
    }

    #[tokio::test]
    async fn test_patch_all_malformed_created_by() {
        let storage = setup_sqlite().await;
        create_test_urls(&storage).await;

        // Patch all malformed URLs
        let count = storage
            .patch_all_malformed_created_by("fixeduser")
            .await
            .unwrap();
        
        // Should have patched 4 malformed entries (2 all-zero UUID, 1 empty string, 1 null)
        assert_eq!(count, 4, "Should have patched exactly 4 malformed URLs");

        // Verify malformed URLs are now fixed
        let url1 = storage.get_authoritative("malformed1").await.unwrap();
        assert_eq!(url1.unwrap().created_by, Some("fixeduser".to_string()));

        let url2 = storage.get_authoritative("malformed2").await.unwrap();
        assert_eq!(url2.unwrap().created_by, Some("fixeduser".to_string()));

        let url3 = storage.get_authoritative("malformed3").await.unwrap();
        assert_eq!(url3.unwrap().created_by, Some("fixeduser".to_string()));

        let url4 = storage.get_authoritative("malformed4").await.unwrap();
        assert_eq!(url4.unwrap().created_by, Some("fixeduser".to_string()));

        // Verify normal URLs are unchanged
        let normal1 = storage.get_authoritative("normal1").await.unwrap();
        assert_eq!(normal1.unwrap().created_by, Some("user123".to_string()));

        let normal2 = storage.get_authoritative("normal2").await.unwrap();
        assert_eq!(normal2.unwrap().created_by, Some("user456".to_string()));
    }

    #[tokio::test]
    async fn test_patch_all_malformed_no_malformed_urls() {
        let storage = setup_sqlite().await;

        // Create only normal URLs
        storage
            .create_with_code("normal1", "https://example.com/1", Some("user123"))
            .await
            .unwrap();
        storage
            .create_with_code("normal2", "https://example.com/2", Some("user456"))
            .await
            .unwrap();

        // Try to patch all malformed URLs
        let count = storage
            .patch_all_malformed_created_by("fixeduser")
            .await
            .unwrap();
        
        assert_eq!(count, 0, "Should have patched 0 URLs when all are normal");

        // Verify URLs are unchanged
        let url1 = storage.get_authoritative("normal1").await.unwrap();
        assert_eq!(url1.unwrap().created_by, Some("user123".to_string()));

        let url2 = storage.get_authoritative("normal2").await.unwrap();
        assert_eq!(url2.unwrap().created_by, Some("user456".to_string()));
    }

    #[tokio::test]
    async fn test_patch_does_not_overwrite_valid_uuids() {
        let storage = setup_sqlite().await;

        // Create URLs with valid UUIDs and other user IDs
        storage
            .create_with_code(
                "valid_uuid",
                "https://example.com/1",
                Some("123e4567-e89b-12d3-a456-426614174000"),
            )
            .await
            .unwrap();

        storage
            .create_with_code(
                "email_user",
                "https://example.com/2",
                Some("user@example.com"),
            )
            .await
            .unwrap();

        storage
            .create_with_code("simple_id", "https://example.com/3", Some("admin"))
            .await
            .unwrap();

        // Create one malformed URL
        storage
            .create_with_code(
                "malformed",
                "https://example.com/4",
                Some("00000000-0000-0000-0000-000000000000"),
            )
            .await
            .unwrap();

        // Patch all malformed URLs
        let count = storage
            .patch_all_malformed_created_by("fixeduser")
            .await
            .unwrap();
        
        assert_eq!(count, 1, "Should have patched only 1 malformed URL");

        // Verify valid UUIDs and other IDs are unchanged
        let url1 = storage.get_authoritative("valid_uuid").await.unwrap();
        assert_eq!(
            url1.unwrap().created_by,
            Some("123e4567-e89b-12d3-a456-426614174000".to_string())
        );

        let url2 = storage.get_authoritative("email_user").await.unwrap();
        assert_eq!(
            url2.unwrap().created_by,
            Some("user@example.com".to_string())
        );

        let url3 = storage.get_authoritative("simple_id").await.unwrap();
        assert_eq!(url3.unwrap().created_by, Some("admin".to_string()));

        // Verify malformed URL is fixed
        let url4 = storage.get_authoritative("malformed").await.unwrap();
        assert_eq!(url4.unwrap().created_by, Some("fixeduser".to_string()));
    }

    #[tokio::test]
    async fn test_patch_single_url_updates_only_target() {
        let storage = setup_sqlite().await;

        // Create multiple malformed URLs
        storage
            .create_with_code(
                "malformed1",
                "https://example.com/1",
                Some("00000000-0000-0000-0000-000000000000"),
            )
            .await
            .unwrap();

        storage
            .create_with_code("malformed2", "https://example.com/2", None)
            .await
            .unwrap();

        storage
            .create_with_code("malformed3", "https://example.com/3", Some(""))
            .await
            .unwrap();

        // Patch only one URL
        let updated = storage
            .patch_created_by("malformed2", "specificuser")
            .await
            .unwrap();
        assert!(updated);

        // Verify only the targeted URL is updated
        let url1 = storage.get_authoritative("malformed1").await.unwrap();
        assert_eq!(
            url1.unwrap().created_by,
            Some("00000000-0000-0000-0000-000000000000".to_string())
        );

        let url2 = storage.get_authoritative("malformed2").await.unwrap();
        assert_eq!(
            url2.unwrap().created_by,
            Some("specificuser".to_string())
        );

        let url3 = storage.get_authoritative("malformed3").await.unwrap();
        assert_eq!(url3.unwrap().created_by, Some("".to_string()));
    }

    #[tokio::test]
    async fn test_list_all_users() {
        let storage = setup_sqlite().await;

        // Create test users
        storage.upsert_user("user1", Some("user1@example.com"), "oauth").await.unwrap();
        storage.upsert_user("user2", Some("user2@example.com"), "oauth").await.unwrap();
        storage.upsert_user("user3", None, "cloudflare").await.unwrap();

        // List all users
        let users = storage.list_all_users(10, 0).await.unwrap();
        assert_eq!(users.len(), 3);

        // Test pagination
        let users_page1 = storage.list_all_users(2, 0).await.unwrap();
        assert_eq!(users_page1.len(), 2);

        let users_page2 = storage.list_all_users(2, 2).await.unwrap();
        assert_eq!(users_page2.len(), 1);
    }

    #[tokio::test]
    async fn test_list_user_links() {
        let storage = setup_sqlite().await;

        // Create test URLs for different users
        storage.create_with_code("link1", "https://example.com/1", Some("user1")).await.unwrap();
        storage.create_with_code("link2", "https://example.com/2", Some("user1")).await.unwrap();
        storage.create_with_code("link3", "https://example.com/3", Some("user2")).await.unwrap();

        // List links for user1
        let links = storage.list_user_links("user1", 10, 0).await.unwrap();
        assert_eq!(links.len(), 2);

        // List links for user2
        let links = storage.list_user_links("user2", 10, 0).await.unwrap();
        assert_eq!(links.len(), 1);

        // List links for non-existent user
        let links = storage.list_user_links("nonexistent", 10, 0).await.unwrap();
        assert_eq!(links.len(), 0);
    }

    #[tokio::test]
    async fn test_bulk_deactivate_user_links() {
        let storage = setup_sqlite().await;

        // Create test URLs for a user
        storage.create_with_code("link1", "https://example.com/1", Some("user1")).await.unwrap();
        storage.create_with_code("link2", "https://example.com/2", Some("user1")).await.unwrap();
        storage.create_with_code("link3", "https://example.com/3", Some("user2")).await.unwrap();

        // Deactivate all links for user1
        let count = storage.bulk_deactivate_user_links("user1").await.unwrap();
        assert_eq!(count, 2);

        // Verify user1's links are deactivated
        let link1 = storage.get_authoritative("link1").await.unwrap().unwrap();
        assert!(!link1.is_active);

        let link2 = storage.get_authoritative("link2").await.unwrap().unwrap();
        assert!(!link2.is_active);

        // Verify user2's link is still active
        let link3 = storage.get_authoritative("link3").await.unwrap().unwrap();
        assert!(link3.is_active);

        // Try to deactivate again (should return 0)
        let count = storage.bulk_deactivate_user_links("user1").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_bulk_reactivate_user_links() {
        let storage = setup_sqlite().await;

        // Create and deactivate test URLs for a user
        storage.create_with_code("link1", "https://example.com/1", Some("user1")).await.unwrap();
        storage.create_with_code("link2", "https://example.com/2", Some("user1")).await.unwrap();
        storage.deactivate("link1").await.unwrap();
        storage.deactivate("link2").await.unwrap();

        // Reactivate all links for user1
        let count = storage.bulk_reactivate_user_links("user1").await.unwrap();
        assert_eq!(count, 2);

        // Verify user1's links are reactivated
        let link1 = storage.get_authoritative("link1").await.unwrap().unwrap();
        assert!(link1.is_active);

        let link2 = storage.get_authoritative("link2").await.unwrap().unwrap();
        assert!(link2.is_active);

        // Try to reactivate again (should return 0)
        let count = storage.bulk_reactivate_user_links("user1").await.unwrap();
        assert_eq!(count, 0);
    }
}

