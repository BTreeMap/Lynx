use crate::analytics::{DEFAULT_IP_VERSION, DROPPED_DIMENSION_MARKER};
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

        // Create analytics table for visitor IP analytics
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS analytics (
                id BIGSERIAL PRIMARY KEY,
                short_code TEXT NOT NULL,
                time_bucket BIGINT NOT NULL,
                country_code TEXT,
                region TEXT,
                city TEXT,
                asn BIGINT,
                ip_version INTEGER NOT NULL,
                visit_count BIGINT NOT NULL DEFAULT 0,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL,
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
            LIMIT $1 OFFSET $2
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
            WHERE created_by = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
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
            SET is_active = false
            WHERE created_by = $1 AND is_active = true
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
            SET is_active = true
            WHERE created_by = $1 AND is_active = false
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
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT(short_code, time_bucket, country_code, region, city, asn, ip_version)
                DO UPDATE SET visit_count = analytics.visit_count + $11, updated_at = $12
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
        let results = if let (Some(start), Some(end)) = (start_time, end_time) {
            sqlx::query_as::<_, crate::analytics::AnalyticsEntry>(
                "SELECT id, short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at FROM analytics WHERE short_code = $1 AND time_bucket >= $2 AND time_bucket <= $3 ORDER BY time_bucket DESC LIMIT $4"
            )
            .bind(short_code)
            .bind(start)
            .bind(end)
            .bind(limit)
            .fetch_all(self.pool.as_ref())
            .await?
        } else if let Some(start) = start_time {
            sqlx::query_as::<_, crate::analytics::AnalyticsEntry>(
                "SELECT id, short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at FROM analytics WHERE short_code = $1 AND time_bucket >= $2 ORDER BY time_bucket DESC LIMIT $3"
            )
            .bind(short_code)
            .bind(start)
            .bind(limit)
            .fetch_all(self.pool.as_ref())
            .await?
        } else if let Some(end) = end_time {
            sqlx::query_as::<_, crate::analytics::AnalyticsEntry>(
                "SELECT id, short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at FROM analytics WHERE short_code = $1 AND time_bucket <= $2 ORDER BY time_bucket DESC LIMIT $3"
            )
            .bind(short_code)
            .bind(end)
            .bind(limit)
            .fetch_all(self.pool.as_ref())
            .await?
        } else {
            sqlx::query_as::<_, crate::analytics::AnalyticsEntry>(
                "SELECT id, short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at FROM analytics WHERE short_code = $1 ORDER BY time_bucket DESC LIMIT $2"
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
                "CASE WHEN region = '<dropped>' THEN region ELSE CONCAT(COALESCE(region, 'Unknown'), ', ', COALESCE(country_code, 'Unknown')) END"
            }
            "city" => {
                // Don't format if city is <dropped>
                "CASE WHEN city = '<dropped>' THEN city ELSE CONCAT(COALESCE(city, 'Unknown'), ', ', COALESCE(region, 'Unknown'), ', ', COALESCE(country_code, 'Unknown')) END"
            }
            "asn" => "CAST(asn AS TEXT)",
            "hour" => "CAST(time_bucket AS TEXT)",
            "day" => "CAST((time_bucket / 86400) * 86400 AS TEXT)",
            _ => "country_code",
        };

        let query_str = if let (Some(_start), Some(_end)) = (start_time, end_time) {
            format!(
                "SELECT {} as dimension, CAST(SUM(visit_count) AS BIGINT) as visit_count FROM analytics WHERE short_code = $1 AND time_bucket >= $2 AND time_bucket <= $3 AND {} IS NOT NULL GROUP BY {} ORDER BY visit_count DESC LIMIT $4",
                group_field, group_field, group_field
            )
        } else if let Some(_start) = start_time {
            format!(
                "SELECT {} as dimension, CAST(SUM(visit_count) AS BIGINT) as visit_count FROM analytics WHERE short_code = $1 AND time_bucket >= $2 AND {} IS NOT NULL GROUP BY {} ORDER BY visit_count DESC LIMIT $3",
                group_field, group_field, group_field
            )
        } else if let Some(_end) = end_time {
            format!(
                "SELECT {} as dimension, CAST(SUM(visit_count) AS BIGINT) as visit_count FROM analytics WHERE short_code = $1 AND time_bucket <= $2 AND {} IS NOT NULL GROUP BY {} ORDER BY visit_count DESC LIMIT $3",
                group_field, group_field, group_field
            )
        } else {
            format!(
                "SELECT {} as dimension, CAST(SUM(visit_count) AS BIGINT) as visit_count FROM analytics WHERE short_code = $1 AND {} IS NOT NULL GROUP BY {} ORDER BY visit_count DESC LIMIT $2",
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
        let cutoff_time = chrono::Utc::now().timestamp() - (retention_days * 86400);

        // Count old entries before pruning
        let count_query = "SELECT COUNT(*)::BIGINT FROM analytics WHERE time_bucket < $1";
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
        let drop_time_bucket = drop_dimensions.contains(&"time_bucket".to_string());
        let drop_country = drop_dimensions.contains(&"country_code".to_string())
            || drop_dimensions.contains(&"country".to_string());

        // Handle time bucket dropping
        // When aggregating old entries, we always need to set time_bucket to a value >= cutoff_time
        // to avoid the aggregated entries being immediately deleted
        let now = chrono::Utc::now().timestamp();
        let time_bucket_expr = if drop_time_bucket {
            // When dropping time_bucket, set all entries to cutoff_time (day boundary)
            format!("CAST({} AS BIGINT)", cutoff_time)
        } else {
            // When keeping time_bucket, use current time to avoid deletion
            // The time dimension info is preserved in the created_at/updated_at timestamps
            format!("CAST({} AS BIGINT)", now)
        };
        select_fields.push(format!("{} as time_bucket", time_bucket_expr));

        // Add dimension fields with conditional dropping
        for field in &["country_code", "region", "city", "asn", "ip_version"] {
            if drop_dimensions.contains(&field.to_string())
                || (field == &"country_code" && drop_country)
            {
                if field == &"asn" {
                    select_fields.push(format!("NULL::BIGINT as {}", field));
                } else if field == &"ip_version" {
                    select_fields.push(format!("{} as {}", DEFAULT_IP_VERSION, field));
                } else {
                    select_fields
                        .push(format!("'{}'::TEXT as {}", DROPPED_DIMENSION_MARKER, field));
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

        // Create a view of aggregated old entries (without inserting yet)
        let mut tx = self.pool.begin().await?;

        // Create aggregated entries
        let aggregate_query = format!(
            "INSERT INTO analytics (short_code, time_bucket, country_code, region, city, asn, ip_version, visit_count, created_at, updated_at)
             SELECT {}, SUM(visit_count)::BIGINT as visit_count, {} as created_at, {} as updated_at
             FROM analytics
             WHERE time_bucket < $1 AND time_bucket != $2
             GROUP BY {}",
            select_clause, now, now, group_by_clause
        );

        let insert_result = sqlx::query(&aggregate_query)
            .bind(cutoff_time)
            .execute(&mut *tx)
            .await?;

        let inserted_count = insert_result.rows_affected() as i64;

        // Delete old entries
        let delete_query = "DELETE FROM analytics WHERE time_bucket < $1";
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

    // Helper to set up a test PostgreSQL database
    // Note: These tests require a running PostgreSQL instance
    // Set DATABASE_URL environment variable to run these tests
    async fn setup_postgres() -> Option<Arc<dyn Storage>> {
        let db_url = std::env::var("DATABASE_URL").ok()?;
        let storage = PostgresStorage::new(&db_url, 5).await.ok()?;
        storage.init().await.ok()?;
        Some(Arc::new(storage))
    }

    #[tokio::test]
    async fn test_analytics_upsert_and_retrieval() {
        let Some(storage) = setup_postgres().await else {
            println!("SKIPPED: DATABASE_URL not set");
            return;
        };

        // Create a short code
        storage
            .create_with_code("test123", "https://example.com", Some("user1"))
            .await
            .unwrap();

        // Insert analytics records
        let time_bucket = 1698768000;
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
        let Some(storage) = setup_postgres().await else {
            println!("SKIPPED: DATABASE_URL not set");
            return;
        };

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
    async fn test_analytics_aggregate_by_asn() {
        let Some(storage) = setup_postgres().await else {
            println!("SKIPPED: DATABASE_URL not set");
            return;
        };

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
        let Some(storage) = setup_postgres().await else {
            println!("SKIPPED: DATABASE_URL not set");
            return;
        };

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

        // Query with start and end time
        let analytics = storage
            .get_analytics("test", Some(1500), Some(2500), 100)
            .await
            .unwrap();
        let total: i64 = analytics.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 3); // Only record from 2000
    }

    #[tokio::test]
    async fn test_analytics_upsert_increments_existing() {
        let Some(storage) = setup_postgres().await else {
            println!("SKIPPED: DATABASE_URL not set");
            return;
        };

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
}
