use crate::models::{ShortenedUrl, UrlHistoryEntry};
use crate::storage::{
    LookupMetadata, LookupResult, OwnedClickError, SearchParams, SearchResult, Storage,
    StorageResult,
};
use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use moka::future::Cache;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, mpsc::error::TrySendError, Mutex};
use tokio::task::JoinHandle;
use tokio::time;

/// Message types for the ClickCounterActor
enum ActorMessage {
    /// Increment a short code's click count by the given amount
    BatchIncrement(String, u64),
    /// Shutdown signal - flush all data
    Shutdown,
}

fn enqueue_click_increment(
    actor_tx: &mpsc::Sender<ActorMessage>,
    read_view: &DashMap<String, u64>,
    short_code: String,
    amount: u64,
) -> Result<(), OwnedClickError> {
    if amount == 0 {
        return Ok(());
    }

    match actor_tx.try_send(ActorMessage::BatchIncrement(short_code, amount)) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(message)) => {
            let ActorMessage::BatchIncrement(short_code, amount) = message else {
                unreachable!("only batch increments are sent by this function")
            };
            read_view
                .entry(short_code)
                .and_modify(|count| *count += amount)
                .or_insert(amount);
            Ok(())
        }
        Err(TrySendError::Closed(message)) => {
            let ActorMessage::BatchIncrement(short_code, _) = message else {
                unreachable!("only batch increments are sent by this function")
            };
            Err(OwnedClickError::new(
                short_code,
                anyhow::anyhow!("click counter actor channel closed"),
            ))
        }
    }
}

/// Actor that manages click counting with a lock-free buffer
struct ClickCounterActor {
    /// Channel receiver for incoming click events
    receiver: mpsc::Receiver<ActorMessage>,
    /// Lock-free HashMap buffer (Layer 1) - single-threaded access only
    buffer: HashMap<String, u64>,
    /// Shared DashMap for concurrent reads (Layer 2)
    read_view: Arc<DashMap<String, u64>>,
    /// Underlying storage for persistence (Layer 3)
    storage: Arc<dyn Storage>,
    /// Fast flush interval (Layer 1 → Layer 2)
    fast_flush_interval: Duration,
    /// Slow flush interval (Layer 2 → Layer 3)
    slow_flush_interval: Duration,
}

impl ClickCounterActor {
    async fn run(mut self) {
        let mut fast_flush_ticker = time::interval(self.fast_flush_interval);
        let mut slow_flush_ticker = time::interval(self.slow_flush_interval);
        let mut flush_tasks = Vec::new();

        // Skip the first tick which fires immediately
        fast_flush_ticker.tick().await;
        slow_flush_ticker.tick().await;

        loop {
            tokio::select! {
                // Handle incoming click events
                Some(msg) = self.receiver.recv() => {
                    match msg {
                        ActorMessage::BatchIncrement(short_code, count) => {
                            // Fast local increment in Layer 1 (no locks!)
                            *self.buffer.entry(short_code).or_insert(0) += count;
                        }
                        ActorMessage::Shutdown => {
                            tracing::info!("Actor received shutdown signal, flushing all data...");
                            // Flush Layer 1 → Layer 2
                            self.flush_buffer_to_read_view();
                            // Flush Layer 2 → Layer 3 and await every in-flight write.
                            if let Some(handle) = self.flush_read_view_to_storage() {
                                flush_tasks.push(handle);
                            }
                            finish_flush_tasks(&mut flush_tasks).await;
                            tracing::info!("All data flushed successfully on shutdown");
                            break;
                        }
                    }
                }
                // Fast flush: Layer 1 → Layer 2 (100ms default)
                _ = fast_flush_ticker.tick() => {
                    self.flush_buffer_to_read_view();
                }
                // Slow flush: Layer 2 → Layer 3 (5s default)
                _ = slow_flush_ticker.tick() => {
                    // Spawns background task, doesn't block the actor
                    if let Some(handle) = self.flush_read_view_to_storage() {
                        flush_tasks.push(handle);
                    }
                    reap_finished_flush_tasks(&mut flush_tasks).await;
                }
                // Channel closed without shutdown message
                else => {
                    tracing::warn!("Actor channel closed unexpectedly, flushing data...");
                    self.flush_buffer_to_read_view();
                    // Flush to storage and await every in-flight write.
                    if let Some(handle) = self.flush_read_view_to_storage() {
                        flush_tasks.push(handle);
                    }
                    finish_flush_tasks(&mut flush_tasks).await;
                    break;
                }
            }
        }
    }

    /// Flush Layer 1 (buffer) → Layer 2 (read_view DashMap)
    /// This is fast and non-blocking
    fn flush_buffer_to_read_view(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        for (short_code, count) in self.buffer.drain() {
            self.read_view
                .entry(short_code)
                .and_modify(|v| *v += count)
                .or_insert(count);
        }
    }

    /// Flush Layer 2 (read_view) → Layer 3 (database)
    /// This can be slow but doesn't block Layer 1 ingestion
    /// Returns a JoinHandle to the background flush task
    fn flush_read_view_to_storage(&self) -> Option<tokio::task::JoinHandle<()>> {
        // Atomically collect and zero out counts from DashMap
        // This is fast and happens synchronously to maintain data consistency
        let pending_updates: Vec<(String, u64)> = self
            .read_view
            .iter_mut()
            .filter_map(|mut entry| {
                let count = *entry.value();
                if count == 0 {
                    return None;
                }
                // Atomically zero the entry - any new increments will be added to 0
                *entry.value_mut() = 0;
                Some((entry.key().clone(), count))
            })
            .collect();

        // Remove zero entries (fast operation)
        self.read_view.retain(|_, v| *v > 0);

        // Skip spawning if there's nothing to flush
        if pending_updates.is_empty() {
            return None;
        }

        // Spawn the slow database writes in a separate task
        // This doesn't block the actor from processing new clicks
        // Return the JoinHandle so callers can optionally wait for completion
        let storage = Arc::clone(&self.storage);
        Some(tokio::spawn(async move {
            for (short_code, count) in pending_updates {
                if let Err(e) = storage.increment_clicks(&short_code, count).await {
                    tracing::error!("Failed to persist click count for '{}': {}", short_code, e);
                }
            }
        }))
    }
}

async fn reap_finished_flush_tasks(flush_tasks: &mut Vec<JoinHandle<()>>) {
    while let Some(index) = flush_tasks.iter().position(JoinHandle::is_finished) {
        let handle = flush_tasks.swap_remove(index);
        if let Err(error) = handle.await {
            tracing::error!(%error, "background click flush task panicked");
        }
    }
}

async fn finish_flush_tasks(flush_tasks: &mut Vec<JoinHandle<()>>) {
    for handle in flush_tasks.drain(..) {
        if let Err(error) = handle.await {
            tracing::error!(%error, "background click flush task panicked during shutdown");
        }
    }
}

/// Cached storage wrapper that implements read caching and write buffering
pub struct CachedStorage {
    /// Underlying storage implementation
    inner: Arc<dyn Storage>,
    /// Read cache for URL lookups (Moka cache)
    read_cache: Cache<String, Option<Arc<ShortenedUrl>>>,
    /// Shared read view for real-time click statistics (Layer 2)
    read_view: Arc<DashMap<String, u64>>,
    /// Actor message sender
    actor_tx: mpsc::Sender<ActorMessage>,
    /// Long-lived actor task, joined during graceful shutdown.
    actor_handle: Mutex<Option<JoinHandle<()>>>,
}

impl CachedStorage {
    pub fn new(
        inner: Arc<dyn Storage>,
        max_cache_entries: u64,
        flush_interval_secs: u64,
        actor_buffer_size: usize,
        actor_flush_interval_ms: u64,
    ) -> Self {
        let read_cache = Cache::builder().max_capacity(max_cache_entries).build();
        let read_view = Arc::new(DashMap::new());

        // Create actor channel with large buffer to prevent message loss
        let (actor_tx, actor_rx) = mpsc::channel(actor_buffer_size);

        // Spawn the click counter actor
        let actor = ClickCounterActor {
            receiver: actor_rx,
            buffer: HashMap::new(),
            read_view: Arc::clone(&read_view),
            storage: Arc::clone(&inner),
            fast_flush_interval: Duration::from_millis(actor_flush_interval_ms),
            slow_flush_interval: Duration::from_secs(flush_interval_secs),
        };

        let actor_handle = tokio::spawn(async move {
            actor.run().await;
        });

        Self {
            inner,
            read_cache,
            read_view,
            actor_tx,
            actor_handle: Mutex::new(Some(actor_handle)),
        }
    }

    /// Flush buffered clicks and wait for the long-lived actor to stop.
    pub async fn shutdown(&self) {
        let mut actor_handle = self.actor_handle.lock().await;
        let Some(actor_handle) = actor_handle.take() else {
            return;
        };

        if let Err(error) = self.actor_tx.send(ActorMessage::Shutdown).await {
            tracing::warn!(%error, "click counter actor stopped before shutdown signal");
        }
        if let Err(error) = actor_handle.await {
            tracing::error!(%error, "click counter actor panicked during shutdown");
        }
    }

    /// Enqueue a click without awaiting channel capacity.
    ///
    /// The bounded actor queue is the uncontended fast path. Saturated queues
    /// merge counts into the actor's existing shared flush layer, preserving
    /// clicks without creating a task per redirect or growing an unbounded queue.
    pub fn buffer_click_owned(
        &self,
        short_code: String,
        amount: u64,
    ) -> Result<(), OwnedClickError> {
        enqueue_click_increment(&self.actor_tx, &self.read_view, short_code, amount)
    }

    /// Get buffered click count for a short code from Layer 2 (read_view)
    fn get_buffered_clicks(&self, short_code: &str) -> u64 {
        self.read_view
            .get(short_code)
            .map(|entry| *entry.value())
            .unwrap_or(0)
    }

    /// Invalidate cache entry for a specific short code
    async fn invalidate_cache(&self, short_code: &str) {
        self.read_cache.invalidate(short_code).await;
    }
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

    async fn get(&self, short_code: &str) -> Result<Option<Arc<ShortenedUrl>>> {
        if let Some(cached) = self.read_cache.get(short_code).await {
            return Ok(cached);
        }

        let url = self.inner.get(short_code).await?;
        self.read_cache
            .insert(short_code.to_string(), url.clone())
            .await;
        Ok(url)
    }

    async fn get_with_metadata(&self, short_code: &str) -> Result<LookupResult> {
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
        let url = self.inner.get(short_code).await?;
        let db_duration = db_start.elapsed();

        // Cache the result from database (for both Some and None)
        self.read_cache
            .insert(short_code.to_string(), url.clone())
            .await;

        Ok(LookupResult {
            url,
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

    async fn update_url(
        &self,
        short_code: &str,
        new_url: &str,
        updated_by: Option<&str>,
    ) -> StorageResult<Option<Arc<ShortenedUrl>>> {
        let result = self
            .inner
            .update_url(short_code, new_url, updated_by)
            .await?;

        // Invalidate cache so the new destination is served immediately
        self.invalidate_cache(short_code).await;

        Ok(result)
    }

    async fn get_url_history(&self, short_code: &str) -> Result<Vec<UrlHistoryEntry>> {
        self.inner.get_url_history(short_code).await
    }

    async fn restore_url(
        &self,
        short_code: &str,
        history_id: i64,
        restored_by: Option<&str>,
    ) -> StorageResult<Option<Arc<ShortenedUrl>>> {
        let result = self
            .inner
            .restore_url(short_code, history_id, restored_by)
            .await?;

        // Invalidate cache so the restored destination is served immediately
        self.invalidate_cache(short_code).await;

        Ok(result)
    }

    async fn increment_clicks(&self, short_code: &str, amount: u64) -> Result<()> {
        self.buffer_click_owned(short_code.to_owned(), amount)
            .map_err(anyhow::Error::from)
    }

    async fn increment_clicks_owned(
        &self,
        short_code: String,
        amount: u64,
    ) -> Result<(), OwnedClickError> {
        self.buffer_click_owned(short_code, amount)
    }

    async fn list_with_cursor(
        &self,
        limit: i64,
        cursor: Option<(i64, i64)>,
        is_admin: bool,
        user_id: Option<&str>,
    ) -> Result<Vec<Arc<ShortenedUrl>>> {
        // Get results from database
        let mut urls = self
            .inner
            .list_with_cursor(limit, cursor, is_admin, user_id)
            .await?;

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

    async fn patch_created_by(&self, short_code: &str, new_created_by: &str) -> Result<bool> {
        // No cache invalidation is needed, as read_cache only needs to ensure the correctness of URL redirects.
        self.inner
            .patch_created_by(short_code, new_created_by)
            .await
    }

    async fn patch_all_malformed_created_by(&self, new_created_by: &str) -> Result<i64> {
        // No cache invalidation is needed, as read_cache only needs to ensure the correctness of URL redirects.
        self.inner
            .patch_all_malformed_created_by(new_created_by)
            .await
    }

    async fn list_all_users(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(String, String, String, i64)>> {
        self.inner.list_all_users(limit, offset).await
    }

    async fn list_user_links(
        &self,
        user_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Arc<ShortenedUrl>>> {
        self.inner.list_user_links(user_id, limit, offset).await
    }

    async fn bulk_deactivate_user_links(&self, user_id: &str) -> Result<i64> {
        // Note: This does not invalidate cache - cache purge happens on instance restart
        self.inner.bulk_deactivate_user_links(user_id).await
    }

    async fn bulk_reactivate_user_links(&self, user_id: &str) -> Result<i64> {
        // Note: This does not invalidate cache - cache purge happens on instance restart
        self.inner.bulk_reactivate_user_links(user_id).await
    }

    async fn upsert_analytics_batch(
        &self,
        records: Vec<crate::analytics::AnalyticsRollup>,
    ) -> Result<()> {
        // Analytics are not cached, pass through to storage
        self.inner.upsert_analytics_batch(records).await
    }

    async fn get_analytics(
        &self,
        short_code: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        limit: i64,
    ) -> Result<Vec<crate::analytics::AnalyticsEntry>> {
        // Analytics are not cached, pass through to storage
        self.inner
            .get_analytics(short_code, start_time, end_time, limit)
            .await
    }

    async fn get_analytics_aggregate(
        &self,
        short_code: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
        group_by: crate::analytics::AnalyticsGroupBy,
        limit: i64,
    ) -> Result<Vec<crate::analytics::AnalyticsAggregate>> {
        // Analytics aggregates are not cached, pass through to storage
        self.inner
            .get_analytics_aggregate(short_code, start_time, end_time, group_by, limit)
            .await
    }

    async fn prune_analytics(
        &self,
        retention_days: i64,
        drop_dimensions: &[String],
    ) -> Result<(i64, i64)> {
        // Pass through to inner storage
        self.inner
            .prune_analytics(retention_days, drop_dimensions)
            .await
    }

    async fn search(
        &self,
        params: &SearchParams,
        is_admin: bool,
        user_id: Option<&str>,
    ) -> Result<SearchResult> {
        // Get search results from database
        let mut result = self.inner.search(params, is_admin, user_id).await?;

        // Add buffered clicks to each URL in the result
        for url in &mut result.items {
            let buffered = self.get_buffered_clicks(&url.short_code);
            if buffered > 0 {
                Arc::make_mut(url).clicks += buffered as i64;
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStorage;

    #[tokio::test]
    async fn owned_click_recovers_code_when_actor_is_closed() {
        let (actor_tx, actor_rx) = mpsc::channel(1);
        drop(actor_rx);
        let read_view = DashMap::new();

        let error =
            enqueue_click_increment(&actor_tx, &read_view, "recover-me".to_owned(), 1).unwrap_err();

        assert_eq!(error.short_code(), "recover-me");
    }

    #[tokio::test]
    async fn zero_owned_click_does_not_require_a_live_actor() {
        let (actor_tx, actor_rx) = mpsc::channel(1);
        drop(actor_rx);
        let read_view = DashMap::new();

        enqueue_click_increment(&actor_tx, &read_view, "no-op".to_owned(), 0).unwrap();
    }

    #[tokio::test]
    async fn full_actor_queue_merges_click_into_flush_layer() {
        let (actor_tx, _actor_rx) = mpsc::channel(1);
        let read_view = DashMap::new();
        actor_tx
            .try_send(ActorMessage::BatchIncrement("queued".to_owned(), 1))
            .unwrap();

        enqueue_click_increment(&actor_tx, &read_view, "overflow".to_owned(), 7).unwrap();

        assert_eq!(read_view.get("overflow").map(|count| *count), Some(7));
    }

    #[tokio::test]
    async fn concurrent_full_queue_merges_every_click() {
        let (actor_tx, _actor_rx) = mpsc::channel(1);
        actor_tx
            .try_send(ActorMessage::BatchIncrement("queued".to_owned(), 1))
            .unwrap();
        let actor_tx = Arc::new(actor_tx);
        let read_view = Arc::new(DashMap::new());
        let mut workers = tokio::task::JoinSet::new();

        for _ in 0..100 {
            let actor_tx = Arc::clone(&actor_tx);
            let read_view = Arc::clone(&read_view);
            workers.spawn(async move {
                enqueue_click_increment(&actor_tx, &read_view, "overflow".to_owned(), 1).unwrap();
            });
        }
        while let Some(result) = workers.join_next().await {
            result.unwrap();
        }

        assert_eq!(read_view.get("overflow").map(|count| *count), Some(100));
    }

    #[tokio::test]
    async fn graceful_shutdown_persists_queued_and_overflow_clicks() {
        let inner = Arc::new(SqliteStorage::new("sqlite::memory:", 1).await.unwrap());
        inner.init().await.unwrap();
        inner
            .create_with_code("durable", "https://example.com", None)
            .await
            .unwrap();
        let storage = CachedStorage::new(inner.clone(), 10, 3_600, 1, 3_600_000);

        storage.buffer_click_owned("durable".to_owned(), 1).unwrap();
        storage.buffer_click_owned("durable".to_owned(), 7).unwrap();
        storage.shutdown().await;

        let url = inner.get_authoritative("durable").await.unwrap().unwrap();
        assert_eq!(url.clicks, 8);
    }
}
