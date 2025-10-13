use crate::models::ShortenedUrl;
use crate::storage::{LookupMetadata, LookupResult, Storage, StorageResult};
use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use moka::future::Cache;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio::time;

/// Cached storage wrapper that implements read caching and write buffering
pub struct CachedStorage {
    /// Underlying storage implementation
    inner: Arc<dyn Storage>,
    /// Read cache for URL lookups (Moka cache)
    read_cache: Cache<String, Option<Arc<ShortenedUrl>>>,
    /// Write buffer for click increments (DashMap)
    click_buffer: Arc<DashMap<String, u64>>,
    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,
}

impl CachedStorage {
    pub fn new(inner: Arc<dyn Storage>, max_cache_entries: u64, flush_interval_secs: u64) -> Self {
        let read_cache = Cache::builder().max_capacity(max_cache_entries).build();

        let click_buffer = Arc::new(DashMap::new());
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        // Start background task to flush click buffer periodically
        let storage = Arc::clone(&inner);
        let buffer = Arc::clone(&click_buffer);
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(flush_interval_secs));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = flush_click_buffer(&storage, &buffer).await {
                            tracing::error!("Failed to flush click buffer: {}", e);
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            tracing::info!("Shutdown signal received, flushing click buffer...");
                            if let Err(e) = flush_click_buffer(&storage, &buffer).await {
                                tracing::error!("Failed to flush click buffer on shutdown: {}", e);
                            } else {
                                tracing::info!("Click buffer flushed successfully on shutdown");
                            }
                            break;
                        }
                    }
                }
            }
        });

        Self {
            inner,
            read_cache,
            click_buffer,
            shutdown_tx,
        }
    }

    /// Signal shutdown to flush buffered data
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Get buffered click count for a short code
    fn get_buffered_clicks(&self, short_code: &str) -> u64 {
        self.click_buffer
            .get(short_code)
            .map(|entry| *entry.value())
            .unwrap_or(0)
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
    // Collect increments while zeroing counts so concurrent writers can continue
    let pending_updates = buffer
        .iter_mut()
        .map_while(|mut entry| {
            let count = *entry.value();
            if count == 0 {
                return None;
            }

            *entry.value_mut() = 0;
            Some((entry.key().clone(), count))
        })
        .collect::<Vec<(String, u64)>>();

    // Remove empty entries in case no new clicks were buffered meanwhile
    buffer.retain(|_, v| *v > 0);

    // Persist updates to the underlying storage
    for (short_code, count) in pending_updates {
        storage.increment_clicks(&short_code, count).await?;
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
    ) -> StorageResult<Arc<ShortenedUrl>> {
        let result = self
            .inner
            .create_with_code(short_code, original_url, created_by)
            .await?;

        // Cache the newly created URL
        self.read_cache
            .insert(short_code.to_string(), Some(Arc::clone(&result)))
            .await;

        Ok(result)
    }

    async fn get(&self, short_code: &str) -> Result<LookupResult> {
        // Try to get from cache first
        let cache_start = Instant::now();
        if let Some(cached) = self.read_cache.get(short_code).await {
            let cache_duration = cache_start.elapsed();
            return Ok(LookupResult {
                url: cached,
                metadata: LookupMetadata {
                    cache_hit: true,
                    cache_duration: Some(cache_duration),
                    db_duration: None,
                },
            });
        }
        let cache_duration = cache_start.elapsed();

        // Cache miss - fetch from underlying storage
        let db_start = Instant::now();
        let result = self.inner.get(short_code).await?;
        let db_duration = db_start.elapsed();

        // Cache the result from database (without buffered clicks to avoid double-counting)
        self.read_cache
            .insert(short_code.to_string(), result.url.clone())
            .await;

        Ok(LookupResult {
            url: result.url,
            metadata: LookupMetadata {
                cache_hit: false,
                cache_duration: Some(cache_duration),
                db_duration: Some(db_duration),
            },
        })
    }

    async fn get_authoritative(&self, short_code: &str) -> Result<Option<Arc<ShortenedUrl>>> {
        let mut result = self.inner.get_authoritative(short_code).await?;

        if let Some(url) = result.as_mut() {
            let buffered = self.get_buffered_clicks(short_code);
            if buffered > 0 {
                Arc::make_mut(url).clicks += buffered as i64;
            }

            self.read_cache
                .insert(short_code.to_string(), Some(Arc::clone(url)))
                .await;
        } else {
            self.read_cache.insert(short_code.to_string(), None).await;
        }

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

    async fn increment_clicks(&self, short_code: &str, amount: u64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }

        // Buffer the click increment in memory
        self.click_buffer
            .entry(short_code.to_string())
            .and_modify(|count| *count += amount)
            .or_insert(amount);

        Ok(())
    }

    async fn list(
        &self,
        limit: i64,
        offset: i64,
        is_admin: bool,
        user_id: Option<&str>,
    ) -> Result<Vec<Arc<ShortenedUrl>>> {
        // Get results from database
        let mut urls = self.inner.list(limit, offset, is_admin, user_id).await?;

        // Add buffered clicks to each URL
        for url in &mut urls {
            let buffered = self.get_buffered_clicks(&url.short_code);
            if buffered > 0 {
                Arc::make_mut(url).clicks += buffered as i64;
            }
        }

        Ok(urls)
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
