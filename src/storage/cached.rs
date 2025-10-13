use crate::models::ShortenedUrl;
use crate::storage::{LookupMetadata, LookupResult, Storage, StorageResult};
use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use moka::future::Cache;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tokio::time;

/// Message types for the ClickCounterActor
enum ActorMessage {
    /// Increment click count for a short code
    IncrementClick(String),
    /// Shutdown signal - flush all data
    Shutdown,
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
        
        // Skip the first tick which fires immediately
        fast_flush_ticker.tick().await;
        slow_flush_ticker.tick().await;
        
        loop {
            tokio::select! {
                // Handle incoming click events
                Some(msg) = self.receiver.recv() => {
                    match msg {
                        ActorMessage::IncrementClick(short_code) => {
                            // Fast local increment in Layer 1 (no locks!)
                            *self.buffer.entry(short_code).or_insert(0) += 1;
                        }
                        ActorMessage::Shutdown => {
                            tracing::info!("Actor received shutdown signal, flushing all data...");
                            // Flush Layer 1 → Layer 2
                            self.flush_buffer_to_read_view();
                            // Flush Layer 2 → Layer 3
                            if let Err(e) = self.flush_read_view_to_storage().await {
                                tracing::error!("Failed to flush to storage during shutdown: {}", e);
                            } else {
                                tracing::info!("All data flushed successfully on shutdown");
                            }
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
                    if let Err(e) = self.flush_read_view_to_storage().await {
                        tracing::error!("Failed to flush read view to storage: {}", e);
                    }
                }
                // Channel closed without shutdown message
                else => {
                    tracing::warn!("Actor channel closed unexpectedly, flushing data...");
                    self.flush_buffer_to_read_view();
                    if let Err(e) = self.flush_read_view_to_storage().await {
                        tracing::error!("Failed to flush to storage on unexpected shutdown: {}", e);
                    }
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
    async fn flush_read_view_to_storage(&self) -> Result<()> {
        // Collect and zero out counts atomically
        let pending_updates: Vec<(String, u64)> = self.read_view
            .iter_mut()
            .filter_map(|mut entry| {
                let count = *entry.value();
                if count == 0 {
                    return None;
                }
                *entry.value_mut() = 0;
                Some((entry.key().clone(), count))
            })
            .collect();
        
        // Remove zero entries
        self.read_view.retain(|_, v| *v > 0);
        
        // Persist to database
        for (short_code, count) in pending_updates {
            self.storage.increment_clicks(&short_code, count).await?;
        }
        
        Ok(())
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
    /// Shutdown signal sender (for backward compatibility)
    shutdown_tx: watch::Sender<bool>,
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
        
        // Backward compatibility shutdown channel
        let (shutdown_tx, _shutdown_rx) = watch::channel(false);
        
        // Spawn the click counter actor
        let actor = ClickCounterActor {
            receiver: actor_rx,
            buffer: HashMap::new(),
            read_view: Arc::clone(&read_view),
            storage: Arc::clone(&inner),
            fast_flush_interval: Duration::from_millis(actor_flush_interval_ms),
            slow_flush_interval: Duration::from_secs(flush_interval_secs),
        };
        
        tokio::spawn(async move {
            actor.run().await;
        });

        Self {
            inner,
            read_cache,
            read_view,
            actor_tx,
            shutdown_tx,
        }
    }

    /// Signal shutdown to flush buffered data
    pub fn shutdown(&self) {
        // Send shutdown message to actor (blocking send to ensure delivery)
        let tx = self.actor_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(ActorMessage::Shutdown).await;
        });
        
        // Also signal the old shutdown channel for backward compatibility
        let _ = self.shutdown_tx.send(true);
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

        // Send click events to actor with blocking send for accuracy
        // Using blocking send provides backpressure instead of dropping messages
        for _ in 0..amount {
            // Clone the sender to avoid holding a reference
            let tx = self.actor_tx.clone();
            let short_code = short_code.to_string();
            
            // Use blocking send to ensure no data loss
            // This applies backpressure if the actor buffer is full
            if let Err(e) = tx.send(ActorMessage::IncrementClick(short_code)).await {
                tracing::error!("Failed to send click to actor (channel closed): {}", e);
                return Err(anyhow::anyhow!("Actor channel closed"));
            }
        }

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
