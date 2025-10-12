use crate::models::ShortenedUrl;
use crate::storage::{Storage, StorageResult};
use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

/// Cached storage wrapper that implements read caching and write buffering
pub struct CachedStorage {
    /// Underlying storage implementation
    inner: Arc<dyn Storage>,
    /// Read cache for URL lookups (Moka cache)
    read_cache: Cache<String, Option<ShortenedUrl>>,
    /// Write buffer for click increments (DashMap)
    click_buffer: Arc<DashMap<String, u64>>,
}

impl CachedStorage {
    pub fn new(inner: Arc<dyn Storage>, max_cache_entries: u64) -> Self {
        let read_cache = Cache::builder()
            .max_capacity(max_cache_entries)
            .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
            .build();

        let click_buffer = Arc::new(DashMap::new());

        // Start background task to flush click buffer periodically
        let storage = Arc::clone(&inner);
        let buffer = Arc::clone(&click_buffer);
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                if let Err(e) = flush_click_buffer(&storage, &buffer).await {
                    tracing::error!("Failed to flush click buffer: {}", e);
                }
            }
        });

        Self {
            inner,
            read_cache,
            click_buffer,
        }
    }

    /// Invalidate cache entry for a specific short code
    async fn invalidate_cache(&self, short_code: &str) {
        self.read_cache.invalidate(short_code).await;
    }
}

/// Flush accumulated clicks to the database
async fn flush_click_buffer(
    storage: &Arc<dyn Storage>,
    buffer: &Arc<DashMap<String, u64>>,
) -> Result<()> {
    // Drain the buffer into a temporary vector to minimize lock time
    let items: Vec<(String, u64)> = buffer
        .iter()
        .map(|entry| (entry.key().clone(), *entry.value()))
        .collect();

    // Clear the buffer
    for (short_code, _) in &items {
        buffer.remove(short_code);
    }

    // Write to database
    for (short_code, count) in items {
        if count > 0 {
            // Execute multiple increments
            for _ in 0..count {
                storage.increment_clicks(&short_code).await?;
            }
        }
    }

    Ok(())
}

#[async_trait]
impl Storage for CachedStorage {
    async fn init(&self) -> Result<()> {
        self.inner.init().await
    }

    async fn create_with_code(
        &self,
        short_code: &str,
        original_url: &str,
        created_by: Option<&str>,
    ) -> StorageResult<ShortenedUrl> {
        let result = self
            .inner
            .create_with_code(short_code, original_url, created_by)
            .await?;

        // Cache the newly created URL
        self.read_cache
            .insert(short_code.to_string(), Some(result.clone()))
            .await;

        Ok(result)
    }

    async fn get(&self, short_code: &str) -> Result<Option<ShortenedUrl>> {
        // Try to get from cache first
        if let Some(cached) = self.read_cache.get(short_code).await {
            return Ok(cached);
        }

        // Cache miss - fetch from underlying storage
        let result = self.inner.get(short_code).await?;

        // Cache the result (including None for non-existent codes)
        self.read_cache
            .insert(short_code.to_string(), result.clone())
            .await;

        Ok(result)
    }

    async fn deactivate(&self, short_code: &str) -> Result<bool> {
        let result = self.inner.deactivate(short_code).await?;

        // Invalidate cache on deactivation
        if result {
            self.invalidate_cache(short_code).await;
        }

        Ok(result)
    }

    async fn reactivate(&self, short_code: &str) -> Result<bool> {
        let result = self.inner.reactivate(short_code).await?;

        // Invalidate cache on reactivation
        if result {
            self.invalidate_cache(short_code).await;
        }

        Ok(result)
    }

    async fn increment_clicks(&self, short_code: &str) -> Result<()> {
        // Buffer the click increment in memory
        self.click_buffer
            .entry(short_code.to_string())
            .and_modify(|count| *count += 1)
            .or_insert(1);

        Ok(())
    }

    async fn list(
        &self,
        limit: i64,
        offset: i64,
        is_admin: bool,
        user_id: Option<&str>,
    ) -> Result<Vec<ShortenedUrl>> {
        // List operations are not cached as they can be large and change frequently
        self.inner.list(limit, offset, is_admin, user_id).await
    }

    async fn upsert_user(
        &self,
        user_id: &str,
        email: Option<&str>,
        auth_method: &str,
    ) -> Result<()> {
        self.inner.upsert_user(user_id, email, auth_method).await
    }

    async fn is_manual_admin(&self, user_id: &str, auth_method: &str) -> Result<bool> {
        self.inner.is_manual_admin(user_id, auth_method).await
    }

    async fn promote_to_admin(&self, user_id: &str, auth_method: &str) -> Result<()> {
        self.inner.promote_to_admin(user_id, auth_method).await
    }

    async fn demote_from_admin(&self, user_id: &str, auth_method: &str) -> Result<bool> {
        self.inner.demote_from_admin(user_id, auth_method).await
    }

    async fn list_manual_admins(&self) -> Result<Vec<(String, String, String)>> {
        self.inner.list_manual_admins().await
    }
}
