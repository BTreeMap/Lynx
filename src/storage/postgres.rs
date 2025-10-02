use crate::models::ShortenedUrl;
use crate::storage::Storage;
use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

pub struct PostgresStorage {
    pool: Arc<PgPool>,
}

impl PostgresStorage {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self {
            pool: Arc::new(pool),
        })
    }
}

#[async_trait]
impl Storage for PostgresStorage {
    async fn init(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS urls (
                id BIGSERIAL PRIMARY KEY,
                short_code TEXT NOT NULL UNIQUE,
                original_url TEXT NOT NULL,
                created_at BIGINT NOT NULL,
                created_by TEXT,
                clicks BIGINT NOT NULL DEFAULT 0,
                expires_at BIGINT
            )
            "#,
        )
        .execute(self.pool.as_ref())
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_short_code ON urls(short_code)")
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    async fn create(
        &self,
        short_code: &str,
        original_url: &str,
        created_by: Option<&str>,
        expires_at: Option<i64>,
    ) -> Result<ShortenedUrl> {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let row = sqlx::query_as::<_, ShortenedUrl>(
            r#"
            INSERT INTO urls (short_code, original_url, created_at, created_by, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, short_code, original_url, created_at, created_by, clicks, expires_at
            "#,
        )
        .bind(short_code)
        .bind(original_url)
        .bind(created_at)
        .bind(created_by)
        .bind(expires_at)
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(row)
    }

    async fn get(&self, short_code: &str) -> Result<Option<ShortenedUrl>> {
        let url = sqlx::query_as::<_, ShortenedUrl>(
            r#"
            SELECT id, short_code, original_url, created_at, created_by, clicks, expires_at
            FROM urls
            WHERE short_code = $1
            "#,
        )
        .bind(short_code)
        .fetch_optional(self.pool.as_ref())
        .await?;

        Ok(url)
    }

    async fn update(
        &self,
        short_code: &str,
        original_url: Option<&str>,
        expires_at: Option<i64>,
    ) -> Result<bool> {
        if let Some(url) = original_url {
            let result = sqlx::query(
                r#"
                UPDATE urls
                SET original_url = $1
                WHERE short_code = $2
                "#,
            )
            .bind(url)
            .bind(short_code)
            .execute(self.pool.as_ref())
            .await?;

            if result.rows_affected() == 0 {
                return Ok(false);
            }
        }

        if let Some(exp) = expires_at {
            sqlx::query(
                r#"
                UPDATE urls
                SET expires_at = $1
                WHERE short_code = $2
                "#,
            )
            .bind(exp)
            .bind(short_code)
            .execute(self.pool.as_ref())
            .await?;
        }

        Ok(true)
    }

    async fn delete(&self, short_code: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM urls
            WHERE short_code = $1
            "#,
        )
        .bind(short_code)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn increment_clicks(&self, short_code: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET clicks = clicks + 1
            WHERE short_code = $1
            "#,
        )
        .bind(short_code)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<ShortenedUrl>> {
        let urls = sqlx::query_as::<_, ShortenedUrl>(
            r#"
            SELECT id, short_code, original_url, created_at, created_by, clicks, expires_at
            FROM urls
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(urls)
    }

    async fn exists(&self, short_code: &str) -> Result<bool> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM urls WHERE short_code = $1
            "#,
        )
        .bind(short_code)
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(count.0 > 0)
    }
}
