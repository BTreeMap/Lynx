use crate::models::ShortenedUrl;
use crate::storage::{LookupMetadata, LookupResult, Storage, StorageError, StorageResult};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::convert::TryFrom;
use std::sync::Arc;
use std::time::Instant;

pub struct PostgresStorage {
    pool: Arc<PgPool>,
}

impl PostgresStorage {
    pub async fn new(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;
        Ok(Self {
            pool: Arc::new(pool),
        })
    }
}

#[async_trait]
impl Storage for PostgresStorage {
    async fn init(&self) -> Result<()> {
        // Create URLs table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS urls (
                id BIGSERIAL PRIMARY KEY,
                short_code TEXT NOT NULL UNIQUE,
                original_url TEXT NOT NULL,
                created_at BIGINT NOT NULL,
                created_by TEXT,
                clicks BIGINT NOT NULL DEFAULT 0,
                is_active BOOLEAN NOT NULL DEFAULT true
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
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL,
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
                promoted_at BIGINT NOT NULL,
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
            VALUES ($1, $2, $3, $4, true)
            ON CONFLICT (short_code) DO NOTHING
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

        let row = sqlx::query_as::<_, ShortenedUrl>(
            r#"
            SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
            FROM urls
            WHERE short_code = $1
            "#,
        )
        .bind(short_code)
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(|e| StorageError::Other(e.into()))?;

        Ok(Arc::new(row))
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
            WHERE short_code = $1
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
            SET is_active = false
            WHERE short_code = $1
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
            SET is_active = true
            WHERE short_code = $1
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
            SET clicks = clicks + $2
            WHERE short_code = $1
            "#,
        )
        .bind(short_code)
        .bind(amount)
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
                    WHERE (created_at, id) < ($1, $2)
                    ORDER BY created_at DESC, id DESC
                    LIMIT $3
                    "#,
                )
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
                    LIMIT $1
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
                    WHERE created_by = $1 AND (created_at, id) < ($2, $3)
                    ORDER BY created_at DESC, id DESC
                    LIMIT $4
                    "#,
                )
                .bind(uid)
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
                    WHERE created_by = $1
                    ORDER BY created_at DESC, id DESC
                    LIMIT $2
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
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_id, auth_method) DO UPDATE SET
                email = COALESCE(EXCLUDED.email, users.email),
                updated_at = EXCLUDED.updated_at
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
            WHERE user_id = $1 AND auth_method = $2
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
            VALUES ($1, $2, $3)
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
            WHERE user_id = $1 AND auth_method = $2
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
            SET created_by = $2
            WHERE short_code = $1
            "#,
        )
        .bind(short_code)
        .bind(new_created_by)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn patch_all_malformed_created_by(&self, new_created_by: &str) -> Result<i64> {
        let result = sqlx::query(
            r#"
            UPDATE urls
            SET created_by = $1
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
}
