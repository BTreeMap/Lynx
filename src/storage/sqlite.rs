use crate::analytics::{DEFAULT_IP_VERSION, DROPPED_DIMENSION_MARKER};
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

        // Create analytics table for visitor IP analytics
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS analytics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                short_code TEXT NOT NULL,
                time_bucket INTEGER NOT NULL,
                country_code TEXT,
                region TEXT,
                city TEXT,
                asn INTEGER,
                ip_version INTEGER NOT NULL,
                visit_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(short_code, time_bucket, country_code, region, city, asn, ip_version)
            )
            "#,
        )
        .execute(self.pool.as_ref())
        .await?;

        // Index for analytics queries by short code
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_analytics_short_code ON analytics(short_code)")
            .execute(self.pool.as_ref())
            .await?;

        // Index for analytics queries by time bucket
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_analytics_time_bucket ON analytics(time_bucket DESC)",
        )
        .execute(self.pool.as_ref())
        .await?;

        // Composite index for short code and time range queries
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_analytics_short_code_time ON analytics(short_code, time_bucket DESC)",
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

    async fn upsert_analytics_batch(
        &self,
        records: Vec<(
            String,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            i32,
            i64,
        )>,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| anyhow!(e))?
            .as_secs() as i64;

        for (short_code, time_bucket, country_code, region, city, asn, ip_version, count) in records
        {
            sqlx::query(
                r#"
                INSERT INTO analytics (short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(short_code, time_bucket, country_code, region, city, asn, ip_version)
                DO UPDATE SET visit_count = visit_count + ?, updated_at = ?
                "#,
            )
            .bind(&short_code)
            .bind(time_bucket)
            .bind(&country_code)
            .bind(&region)
            .bind(&city)
            .bind(asn)
            .bind(ip_version)
            .bind(count)
            .bind(now)
            .bind(now)
            .bind(count)
            .bind(now)
            .execute(self.pool.as_ref())
            .await?;
        }

        Ok(())
    }

    async fn get_analytics(
        &self,
        short_code: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: i64,
    ) -> Result<Vec<crate::analytics::AnalyticsEntry>> {
        // Simplified query building
        let results = if let (Some(start), Some(end)) = (start_time, end_time) {
            sqlx::query_as::<_, crate::analytics::AnalyticsEntry>(
                "SELECT id, short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at FROM analytics WHERE short_code = ? AND time_bucket >= ? AND time_bucket <= ? ORDER BY time_bucket DESC LIMIT ?"
            )
            .bind(short_code)
            .bind(start)
            .bind(end)
            .bind(limit)
            .fetch_all(self.pool.as_ref())
            .await?
        } else if let Some(start) = start_time {
            sqlx::query_as::<_, crate::analytics::AnalyticsEntry>(
                "SELECT id, short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at FROM analytics WHERE short_code = ? AND time_bucket >= ? ORDER BY time_bucket DESC LIMIT ?"
            )
            .bind(short_code)
            .bind(start)
            .bind(limit)
            .fetch_all(self.pool.as_ref())
            .await?
        } else if let Some(end) = end_time {
            sqlx::query_as::<_, crate::analytics::AnalyticsEntry>(
                "SELECT id, short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at FROM analytics WHERE short_code = ? AND time_bucket <= ? ORDER BY time_bucket DESC LIMIT ?"
            )
            .bind(short_code)
            .bind(end)
            .bind(limit)
            .fetch_all(self.pool.as_ref())
            .await?
        } else {
            sqlx::query_as::<_, crate::analytics::AnalyticsEntry>(
                "SELECT id, short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at FROM analytics WHERE short_code = ? ORDER BY time_bucket DESC LIMIT ?"
            )
            .bind(short_code)
            .bind(limit)
            .fetch_all(self.pool.as_ref())
            .await?
        };

        Ok(results)
    }

    async fn get_analytics_aggregate(
        &self,
        short_code: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        group_by: &str,
        limit: i64,
    ) -> Result<Vec<crate::analytics::AnalyticsAggregate>> {
        let group_field = match group_by {
            "country" => "country_code",
            "region" => {
                // Don't format if region is <dropped>
                "CASE WHEN region = '<dropped>' THEN region ELSE COALESCE(region, 'Unknown') || ', ' || COALESCE(country_code, 'Unknown') END"
            }
            "city" => {
                // Don't format if city is <dropped>
                "CASE WHEN city = '<dropped>' THEN city ELSE COALESCE(city, 'Unknown') || ', ' || COALESCE(region, 'Unknown') || ', ' || COALESCE(country_code, 'Unknown') END"
            }
            "asn" => "CAST(asn AS TEXT)",
            "hour" => "time_bucket",
            "day" => "(time_bucket / 86400) * 86400",
            _ => "country_code",
        };

        let query_str = if let (Some(_start), Some(_end)) = (start_time, end_time) {
            format!(
                "SELECT {} as dimension, CAST(SUM(visit_count) AS INTEGER) as visit_count FROM analytics WHERE short_code = ? AND time_bucket >= ? AND time_bucket <= ? AND {} IS NOT NULL GROUP BY {} ORDER BY visit_count DESC LIMIT ?",
                group_field, group_field, group_field
            )
        } else if let Some(_start) = start_time {
            format!(
                "SELECT {} as dimension, CAST(SUM(visit_count) AS INTEGER) as visit_count FROM analytics WHERE short_code = ? AND time_bucket >= ? AND {} IS NOT NULL GROUP BY {} ORDER BY visit_count DESC LIMIT ?",
                group_field, group_field, group_field
            )
        } else if let Some(_end) = end_time {
            format!(
                "SELECT {} as dimension, CAST(SUM(visit_count) AS INTEGER) as visit_count FROM analytics WHERE short_code = ? AND time_bucket <= ? AND {} IS NOT NULL GROUP BY {} ORDER BY visit_count DESC LIMIT ?",
                group_field, group_field, group_field
            )
        } else {
            format!(
                "SELECT {} as dimension, CAST(SUM(visit_count) AS INTEGER) as visit_count FROM analytics WHERE short_code = ? AND {} IS NOT NULL GROUP BY {} ORDER BY visit_count DESC LIMIT ?",
                group_field, group_field, group_field
            )
        };

        let results = if let (Some(start), Some(end)) = (start_time, end_time) {
            sqlx::query_as::<_, crate::analytics::AnalyticsAggregate>(&query_str)
                .bind(short_code)
                .bind(start)
                .bind(end)
                .bind(limit)
                .fetch_all(self.pool.as_ref())
                .await?
        } else if let Some(start) = start_time {
            sqlx::query_as::<_, crate::analytics::AnalyticsAggregate>(&query_str)
                .bind(short_code)
                .bind(start)
                .bind(limit)
                .fetch_all(self.pool.as_ref())
                .await?
        } else if let Some(end) = end_time {
            sqlx::query_as::<_, crate::analytics::AnalyticsAggregate>(&query_str)
                .bind(short_code)
                .bind(end)
                .bind(limit)
                .fetch_all(self.pool.as_ref())
                .await?
        } else {
            sqlx::query_as::<_, crate::analytics::AnalyticsAggregate>(&query_str)
                .bind(short_code)
                .bind(limit)
                .fetch_all(self.pool.as_ref())
                .await?
        };

        Ok(results)
    }

    async fn prune_analytics(
        &self,
        retention_days: i64,
        drop_dimensions: &[String],
    ) -> Result<(i64, i64)> {
        // Compute cutoff_time and round to the start of an hour
        // This ensures time_bucket is always a valid hourly boundary
        let raw_cutoff_time = chrono::Utc::now().timestamp() - (retention_days * 86400);
        let cutoff_time = (raw_cutoff_time / 3600) * 3600;

        // Count old entries before pruning
        let count_query = "SELECT COUNT(*) FROM analytics WHERE time_bucket < ?";
        let old_count: (i64,) = sqlx::query_as(count_query)
            .bind(cutoff_time)
            .fetch_one(self.pool.as_ref())
            .await?;
        let deleted_count = old_count.0;

        // If no old entries, return early
        if deleted_count == 0 {
            return Ok((0, 0));
        }

        // Build the SELECT clause with dropped dimensions replaced
        let mut select_fields = vec!["short_code".to_string()];

        // Pre-compute dimension checks for efficiency
        let drop_country = drop_dimensions.contains(&"country_code".to_string())
            || drop_dimensions.contains(&"country".to_string());

        // Always set time_bucket to cutoff_time for aggregated entries
        // This ensures they won't be immediately deleted and simplifies logic
        select_fields.push(format!("CAST({} AS INTEGER) as time_bucket", cutoff_time));

        // Add dimension fields with conditional dropping
        for field in &["country_code", "region", "city", "asn", "ip_version"] {
            if drop_dimensions.contains(&field.to_string())
                || (field == &"country_code" && drop_country)
            {
                if field == &"asn" {
                    select_fields.push(format!("NULL as {}", field));
                } else if field == &"ip_version" {
                    select_fields.push(format!("{} as {}", DEFAULT_IP_VERSION, field));
                } else {
                    select_fields.push(format!("'{}' as {}", DROPPED_DIMENSION_MARKER, field));
                }
            } else {
                select_fields.push(field.to_string());
            }
        }

        // Build SELECT clause
        let select_clause = select_fields.join(", ");

        // Build GROUP BY clause - use expressions without "as alias" parts
        let group_by_expressions: Vec<String> = select_fields
            .iter()
            .map(|f| {
                // Extract expression before "as" if present, otherwise use the whole field
                if f.contains(" as ") {
                    f.split(" as ").next().unwrap().to_string()
                } else {
                    f.clone()
                }
            })
            .collect();
        let group_by_clause = group_by_expressions.join(", ");

        let now = chrono::Utc::now().timestamp();
        let mut tx = self.pool.begin().await?;

        // Create aggregated entries with time_bucket set to cutoff_time
        let aggregate_query = format!(
            "INSERT INTO analytics (short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at)
             SELECT {}, SUM(visit_count) as visit_count, {} as created_at, {} as updated_at
             FROM analytics
             WHERE time_bucket < ?
             GROUP BY {}",
            select_clause, now, now, group_by_clause
        );

        let insert_result = sqlx::query(&aggregate_query)
            .bind(cutoff_time)
            .execute(&mut *tx)
            .await?;

        let inserted_count = insert_result.rows_affected() as i64;

        // Delete old entries
        let delete_query = "DELETE FROM analytics WHERE time_bucket < ?";
        sqlx::query(delete_query)
            .bind(cutoff_time)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok((deleted_count, inserted_count))
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
        assert_eq!(url2.unwrap().created_by, Some("specificuser".to_string()));

        let url3 = storage.get_authoritative("malformed3").await.unwrap();
        assert_eq!(url3.unwrap().created_by, Some("".to_string()));
    }

    #[tokio::test]
    async fn test_list_all_users() {
        let storage = setup_sqlite().await;

        // Create test users
        storage
            .upsert_user("user1", Some("user1@example.com"), "oauth")
            .await
            .unwrap();
        storage
            .upsert_user("user2", Some("user2@example.com"), "oauth")
            .await
            .unwrap();
        storage
            .upsert_user("user3", None, "cloudflare")
            .await
            .unwrap();

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
        storage
            .create_with_code("link1", "https://example.com/1", Some("user1"))
            .await
            .unwrap();
        storage
            .create_with_code("link2", "https://example.com/2", Some("user1"))
            .await
            .unwrap();
        storage
            .create_with_code("link3", "https://example.com/3", Some("user2"))
            .await
            .unwrap();

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
        storage
            .create_with_code("link1", "https://example.com/1", Some("user1"))
            .await
            .unwrap();
        storage
            .create_with_code("link2", "https://example.com/2", Some("user1"))
            .await
            .unwrap();
        storage
            .create_with_code("link3", "https://example.com/3", Some("user2"))
            .await
            .unwrap();

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
        storage
            .create_with_code("link1", "https://example.com/1", Some("user1"))
            .await
            .unwrap();
        storage
            .create_with_code("link2", "https://example.com/2", Some("user1"))
            .await
            .unwrap();
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

    #[tokio::test]
    async fn test_analytics_upsert_and_retrieval() {
        let storage = setup_sqlite().await;

        // Create a short code
        storage
            .create_with_code("test123", "https://example.com", Some("user1"))
            .await
            .unwrap();

        // Insert analytics records
        let time_bucket = 1698768000; // Some timestamp
        let records = vec![
            (
                "test123".to_string(),
                time_bucket,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("San Francisco".to_string()),
                Some(15169),
                4,
                5,
            ),
            (
                "test123".to_string(),
                time_bucket,
                Some("GB".to_string()),
                Some("England".to_string()),
                Some("London".to_string()),
                Some(16509),
                4,
                3,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Retrieve analytics
        let analytics = storage
            .get_analytics("test123", None, None, 100)
            .await
            .unwrap();

        assert_eq!(analytics.len(), 2);

        // Verify visit counts
        let total_visits: i64 = analytics.iter().map(|a| a.visit_count).sum();
        assert_eq!(total_visits, 8);
    }

    #[tokio::test]
    async fn test_analytics_aggregate_by_country() {
        let storage = setup_sqlite().await;

        // Create a short code
        storage
            .create_with_code("multi", "https://example.com", Some("user1"))
            .await
            .unwrap();

        // Insert analytics from multiple countries
        let time_bucket = 1698768000;
        let records = vec![
            (
                "multi".to_string(),
                time_bucket,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("SF".to_string()),
                Some(15169),
                4,
                10,
            ),
            (
                "multi".to_string(),
                time_bucket,
                Some("GB".to_string()),
                Some("England".to_string()),
                Some("London".to_string()),
                Some(16509),
                4,
                5,
            ),
            (
                "multi".to_string(),
                time_bucket,
                Some("US".to_string()),
                Some("NY".to_string()),
                Some("NYC".to_string()),
                Some(15169),
                4,
                3,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Aggregate by country
        let aggregates = storage
            .get_analytics_aggregate("multi", None, None, "country", 10)
            .await
            .unwrap();

        assert_eq!(aggregates.len(), 2); // US and GB

        // Verify totals
        let total: i64 = aggregates.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 18);

        // Find US aggregate (should be 13 = 10 + 3)
        let us_agg = aggregates.iter().find(|a| a.dimension == "US").unwrap();
        assert_eq!(us_agg.visit_count, 13);

        // Find GB aggregate (should be 5)
        let gb_agg = aggregates.iter().find(|a| a.dimension == "GB").unwrap();
        assert_eq!(gb_agg.visit_count, 5);
    }

    #[tokio::test]
    async fn test_analytics_aggregate_by_region() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        let time_bucket = 1698768000;
        let records = vec![
            (
                "test".to_string(),
                time_bucket,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("LA".to_string()),
                None,
                4,
                7,
            ),
            (
                "test".to_string(),
                time_bucket,
                Some("US".to_string()),
                Some("NY".to_string()),
                Some("NYC".to_string()),
                None,
                4,
                4,
            ),
            (
                "test".to_string(),
                time_bucket,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("SF".to_string()),
                None,
                4,
                2,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Aggregate by region
        let aggregates = storage
            .get_analytics_aggregate("test", None, None, "region", 10)
            .await
            .unwrap();

        assert_eq!(aggregates.len(), 2); // CA and NY

        let total: i64 = aggregates.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 13);

        // CA should have 9 visits (7 + 2), formatted as "CA, US"
        let ca_agg = aggregates.iter().find(|a| a.dimension == "CA, US").unwrap();
        assert_eq!(ca_agg.visit_count, 9);
    }

    #[tokio::test]
    async fn test_analytics_aggregate_by_asn() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        let time_bucket = 1698768000;
        let records = vec![
            (
                "test".to_string(),
                time_bucket,
                Some("US".to_string()),
                None,
                None,
                Some(15169),
                4,
                8,
            ),
            (
                "test".to_string(),
                time_bucket,
                Some("US".to_string()),
                None,
                None,
                Some(16509),
                4,
                3,
            ),
            (
                "test".to_string(),
                time_bucket,
                Some("GB".to_string()),
                None,
                None,
                Some(15169),
                4,
                2,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Aggregate by ASN
        let aggregates = storage
            .get_analytics_aggregate("test", None, None, "asn", 10)
            .await
            .unwrap();

        assert_eq!(aggregates.len(), 2);

        let total: i64 = aggregates.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 13);

        // ASN 15169 should have 10 visits (8 + 2)
        let asn_agg = aggregates.iter().find(|a| a.dimension == "15169").unwrap();
        assert_eq!(asn_agg.visit_count, 10);
    }

    #[tokio::test]
    async fn test_analytics_time_range_filtering() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        // Insert records with different time buckets
        let records = vec![
            (
                "test".to_string(),
                1000,
                Some("US".to_string()),
                None,
                None,
                None,
                4,
                5,
            ),
            (
                "test".to_string(),
                2000,
                Some("US".to_string()),
                None,
                None,
                None,
                4,
                3,
            ),
            (
                "test".to_string(),
                3000,
                Some("US".to_string()),
                None,
                None,
                None,
                4,
                7,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Query with start time
        let analytics = storage
            .get_analytics("test", Some(1500), None, 100)
            .await
            .unwrap();
        let total: i64 = analytics.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 10); // Only records from 2000 and 3000

        // Query with end time
        let analytics = storage
            .get_analytics("test", None, Some(2500), 100)
            .await
            .unwrap();
        let total: i64 = analytics.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 8); // Only records from 1000 and 2000

        // Query with both start and end time
        let analytics = storage
            .get_analytics("test", Some(1500), Some(2500), 100)
            .await
            .unwrap();
        let total: i64 = analytics.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 3); // Only record from 2000
    }

    #[tokio::test]
    async fn test_analytics_upsert_increments_existing() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        let time_bucket = 1698768000;

        // Insert initial record - all fields need to match for the UNIQUE constraint to trigger
        let records = vec![(
            "test".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("CA".to_string()),
            Some("SF".to_string()),
            Some(15169),
            4,
            5,
        )];
        storage.upsert_analytics_batch(records).await.unwrap();

        // Upsert same key with more visits - all fields must match
        let records = vec![(
            "test".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("CA".to_string()),
            Some("SF".to_string()),
            Some(15169),
            4,
            3,
        )];
        storage.upsert_analytics_batch(records).await.unwrap();

        // Should have incremented, not replaced
        let analytics = storage
            .get_analytics("test", None, None, 100)
            .await
            .unwrap();

        assert_eq!(analytics.len(), 1);
        assert_eq!(analytics[0].visit_count, 8); // 5 + 3
    }

    #[tokio::test]
    async fn test_analytics_aggregate_with_time_range() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        // Insert records with different time buckets
        let records = vec![
            (
                "test".to_string(),
                1000,
                Some("US".to_string()),
                None,
                None,
                None,
                4,
                5,
            ),
            (
                "test".to_string(),
                2000,
                Some("GB".to_string()),
                None,
                None,
                None,
                4,
                3,
            ),
            (
                "test".to_string(),
                3000,
                Some("US".to_string()),
                None,
                None,
                None,
                4,
                7,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Aggregate with time range that includes only middle record
        let aggregates = storage
            .get_analytics_aggregate("test", Some(1500), Some(2500), "country", 10)
            .await
            .unwrap();

        assert_eq!(aggregates.len(), 1);
        assert_eq!(aggregates[0].dimension, "GB");
        assert_eq!(aggregates[0].visit_count, 3);
    }

    #[tokio::test]
    async fn test_analytics_aggregate_by_city_formatting() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        let time_bucket = 1698768000;
        let records = vec![
            // Full geo data: city, region, country
            (
                "test".to_string(),
                time_bucket,
                Some("CA".to_string()),
                Some("Ontario".to_string()),
                Some("Toronto".to_string()),
                None,
                4,
                5,
            ),
            // Same city, different count
            (
                "test".to_string(),
                time_bucket,
                Some("CA".to_string()),
                Some("Ontario".to_string()),
                Some("Toronto".to_string()),
                None,
                4,
                3,
            ),
            // Different city, same region and country
            (
                "test".to_string(),
                time_bucket,
                Some("CA".to_string()),
                Some("Ontario".to_string()),
                Some("Ottawa".to_string()),
                None,
                4,
                2,
            ),
            // Missing region
            (
                "test".to_string(),
                time_bucket,
                Some("US".to_string()),
                None,
                Some("Portland".to_string()),
                None,
                4,
                1,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Aggregate by city
        let aggregates = storage
            .get_analytics_aggregate("test", None, None, "city", 10)
            .await
            .unwrap();

        assert_eq!(aggregates.len(), 3);

        let total: i64 = aggregates.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 11);

        // Toronto should be "Toronto, Ontario, CA" with 8 visits (5 + 3)
        let toronto_agg = aggregates
            .iter()
            .find(|a| a.dimension == "Toronto, Ontario, CA")
            .unwrap();
        assert_eq!(toronto_agg.visit_count, 8);

        // Ottawa should be "Ottawa, Ontario, CA" with 2 visits
        let ottawa_agg = aggregates
            .iter()
            .find(|a| a.dimension == "Ottawa, Ontario, CA")
            .unwrap();
        assert_eq!(ottawa_agg.visit_count, 2);

        // Portland should be "Portland, Unknown, US" (missing region)
        let portland_agg = aggregates
            .iter()
            .find(|a| a.dimension == "Portland, Unknown, US")
            .unwrap();
        assert_eq!(portland_agg.visit_count, 1);
    }

    #[tokio::test]
    async fn test_analytics_aggregate_by_region_formatting() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        let time_bucket = 1698768000;
        let records = vec![
            // Full data: country and region
            (
                "test".to_string(),
                time_bucket,
                Some("CA".to_string()),
                Some("Ontario".to_string()),
                Some("Toronto".to_string()),
                None,
                4,
                5,
            ),
            (
                "test".to_string(),
                time_bucket,
                Some("CA".to_string()),
                Some("Ontario".to_string()),
                Some("Ottawa".to_string()),
                None,
                4,
                3,
            ),
            // Different region, same country
            (
                "test".to_string(),
                time_bucket,
                Some("CA".to_string()),
                Some("Quebec".to_string()),
                Some("Montreal".to_string()),
                None,
                4,
                2,
            ),
            // Missing country (edge case)
            (
                "test".to_string(),
                time_bucket,
                None,
                Some("Texas".to_string()),
                Some("Austin".to_string()),
                None,
                4,
                1,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Aggregate by region
        let aggregates = storage
            .get_analytics_aggregate("test", None, None, "region", 10)
            .await
            .unwrap();

        assert_eq!(aggregates.len(), 3);

        let total: i64 = aggregates.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 11);

        // Ontario should be "Ontario, CA" with 8 visits (5 + 3)
        let ontario_agg = aggregates
            .iter()
            .find(|a| a.dimension == "Ontario, CA")
            .unwrap();
        assert_eq!(ontario_agg.visit_count, 8);

        // Quebec should be "Quebec, CA" with 2 visits
        let quebec_agg = aggregates
            .iter()
            .find(|a| a.dimension == "Quebec, CA")
            .unwrap();
        assert_eq!(quebec_agg.visit_count, 2);

        // Texas should be "Texas, Unknown" (missing country)
        let texas_agg = aggregates
            .iter()
            .find(|a| a.dimension == "Texas, Unknown")
            .unwrap();
        assert_eq!(texas_agg.visit_count, 1);
    }

    #[tokio::test]
    async fn test_analytics_prune_drops_dimensions() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        // Create analytics data older than 30 days
        let old_time = chrono::Utc::now().timestamp() - (40 * 86400); // 40 days ago
        let recent_time = chrono::Utc::now().timestamp() - (10 * 86400); // 10 days ago

        let records = vec![
            // Old records (will be pruned)
            (
                "test".to_string(),
                old_time,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("SF".to_string()),
                Some(15169),
                4,
                5,
            ),
            (
                "test".to_string(),
                old_time + 3600,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("LA".to_string()),
                Some(15169),
                4,
                3,
            ),
            (
                "test".to_string(),
                old_time,
                Some("GB".to_string()),
                Some("England".to_string()),
                Some("London".to_string()),
                Some(16509),
                4,
                2,
            ),
            // Recent records (will not be pruned)
            (
                "test".to_string(),
                recent_time,
                Some("CA".to_string()),
                Some("Ontario".to_string()),
                Some("Toronto".to_string()),
                None,
                4,
                4,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Prune with dropping city and region dimensions
        let (deleted, _inserted) = storage
            .prune_analytics(30, &vec!["city".to_string(), "region".to_string()])
            .await
            .unwrap();

        assert_eq!(deleted, 3, "Should have deleted 3 old entries");
        // Aggregated entries with current timestamp are created and should remain
        // (they won't be deleted because their time_bucket is current time > cutoff)
    }

    #[tokio::test]
    async fn test_analytics_prune_sets_time_bucket_to_cutoff() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        // Create analytics data with different hours
        let old_time = chrono::Utc::now().timestamp() - (40 * 86400); // 40 days ago

        let records = vec![
            (
                "test".to_string(),
                old_time,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("SF".to_string()),
                None,
                4,
                5,
            ),
            (
                "test".to_string(),
                old_time + 3600,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("SF".to_string()),
                None,
                4,
                3,
            ), // Different hour
            (
                "test".to_string(),
                old_time + 7200,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("SF".to_string()),
                None,
                4,
                2,
            ), // Another hour
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Prune (time_bucket is always set to cutoff_time now)
        let (deleted, inserted) = storage
            .prune_analytics(30, &vec![])
            .await
            .unwrap();

        assert_eq!(deleted, 3, "Should have deleted 3 old entries");
        assert_eq!(inserted, 1, "Should have created 1 aggregated entry with all data");
        
        // Verify the aggregated entry has time_bucket set to cutoff_time
        let analytics = storage.get_analytics("test", None, None, 100).await.unwrap();
        assert_eq!(analytics.len(), 1);
        assert_eq!(analytics[0].visit_count, 10); // 5 + 3 + 2
        
        // The time_bucket should be at the cutoff_time (rounded to hour start)
        let raw_cutoff = chrono::Utc::now().timestamp() - (30 * 86400);
        let expected_cutoff = (raw_cutoff / 3600) * 3600;
        assert_eq!(analytics[0].time_bucket, expected_cutoff);
    }

    #[tokio::test]
    async fn test_analytics_aggregate_handles_dropped_markers() {
        let storage = setup_sqlite().await;

        storage
            .create_with_code("test", "https://example.com", Some("user1"))
            .await
            .unwrap();

        let time_bucket = 1698768000;
        let records = vec![
            // Normal entry
            (
                "test".to_string(),
                time_bucket,
                Some("US".to_string()),
                Some("CA".to_string()),
                Some("SF".to_string()),
                None,
                4,
                5,
            ),
            // Pruned entry with <dropped> markers
            (
                "test".to_string(),
                time_bucket,
                Some("US".to_string()),
                Some("<dropped>".to_string()),
                Some("<dropped>".to_string()),
                None,
                4,
                3,
            ),
        ];

        storage.upsert_analytics_batch(records).await.unwrap();

        // Aggregate by city
        let city_aggregates = storage
            .get_analytics_aggregate("test", None, None, "city", 10)
            .await
            .unwrap();

        assert_eq!(city_aggregates.len(), 2);

        // Normal entry should be formatted as "SF, CA, US"
        let sf_agg = city_aggregates.iter().find(|a| a.dimension == "SF, CA, US");
        assert!(sf_agg.is_some(), "Should have SF, CA, US entry");
        assert_eq!(sf_agg.unwrap().visit_count, 5);

        // Dropped entry should remain as "<dropped>"
        let dropped_agg = city_aggregates.iter().find(|a| a.dimension == "<dropped>");
        assert!(dropped_agg.is_some(), "Should have <dropped> entry");
        assert_eq!(dropped_agg.unwrap().visit_count, 3);

        // Aggregate by region
        let region_aggregates = storage
            .get_analytics_aggregate("test", None, None, "region", 10)
            .await
            .unwrap();

        assert_eq!(region_aggregates.len(), 2);

        // Normal entry should be formatted as "CA, US"
        let ca_agg = region_aggregates.iter().find(|a| a.dimension == "CA, US");
        assert!(ca_agg.is_some(), "Should have CA, US entry");

        // Dropped entry should remain as "<dropped>"
        let dropped_region_agg = region_aggregates
            .iter()
            .find(|a| a.dimension == "<dropped>");
        assert!(
            dropped_region_agg.is_some(),
            "Should have <dropped> entry for region"
        );
    }
}
