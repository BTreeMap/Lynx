use crate::models::ShortenedUrl;
use crate::storage::Storage;
use anyhow::Result;
use async_trait::async_trait;
use sqlx::SqlitePool;
use std::sync::Arc;

pub struct SqliteStorage {
    pool: Arc<SqlitePool>,
}

impl SqliteStorage {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Self {
            pool: Arc::new(pool),
        })
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn init(&self) -> Result<()> {
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

        Ok(())
    }

    async fn create(
        &self,
        short_code: &str,
        original_url: &str,
        created_by: Option<&str>,
    ) -> Result<ShortenedUrl> {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let id = sqlx::query(
            r#"
            INSERT INTO urls (short_code, original_url, created_at, created_by, is_active)
            VALUES (?, ?, ?, ?, 1)
            "#,
        )
        .bind(short_code)
        .bind(original_url)
        .bind(created_at)
        .bind(created_by)
        .execute(self.pool.as_ref())
        .await?
        .last_insert_rowid();

        Ok(ShortenedUrl {
            id,
            short_code: short_code.to_string(),
            original_url: original_url.to_string(),
            created_at,
            created_by: created_by.map(|s| s.to_string()),
            clicks: 0,
            is_active: true,
        })
    }

    async fn get(&self, short_code: &str) -> Result<Option<ShortenedUrl>> {
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

        Ok(url)
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

    async fn increment_clicks(&self, short_code: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE urls
            SET clicks = clicks + 1
            WHERE short_code = ?
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
            SELECT id, short_code, original_url, created_at, created_by, clicks, is_active
            FROM urls
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
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
            SELECT COUNT(*) FROM urls WHERE short_code = ?
            "#,
        )
        .bind(short_code)
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(count.0 > 0)
    }
}
