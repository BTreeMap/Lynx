//! In-memory analytics aggregator with periodic flush
//!
//! This module provides high-performance in-memory aggregation of
//! analytics records that are periodically flushed to the database.
//!
//! PERFORMANCE OPTIMIZATION: GeoIP lookups are deferred until flush time
//! to keep the hot path (request handling) as fast as possible.
//!
//! Uses actor pattern with mpsc channel to avoid lock contention on hot keys,
//! similar to the ClickCounterActor in storage/cached.rs.

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

use crate::analytics::models::{AnalyticsKey, AnalyticsEvent, AnalyticsRecord, AnalyticsValue};

/// Message types for the AnalyticsActor
enum ActorMessage {
    /// Record an analytics event
    RecordEvent(AnalyticsEvent),
    /// Shutdown signal - flush all data
    Shutdown,
}

/// Actor that manages analytics event buffering with zero lock contention
/// 
/// Uses a 2-layer architecture:
/// - Layer 1: Local HashMap (single-threaded, no locks)
/// - Layer 2: Shared DashMap (for concurrent flush access)
struct AnalyticsActor {
    /// Channel receiver for incoming analytics events
    receiver: mpsc::Receiver<ActorMessage>,
    /// Layer 1: Lock-free event buffer (single-threaded access only in actor)
    buffer: HashMap<String, Vec<AnalyticsEvent>>,
    /// Layer 2: Shared buffer for concurrent reads during flush
    shared_buffer: Arc<DashMap<String, Vec<AnalyticsEvent>>>,
    /// Fast flush interval (Layer 1 → Layer 2)
    fast_flush_interval: Duration,
}

impl AnalyticsActor {
    async fn run(mut self) {
        let mut fast_flush_ticker = tokio::time::interval(self.fast_flush_interval);
        
        // Skip the first tick which fires immediately
        fast_flush_ticker.tick().await;
        
        loop {
            tokio::select! {
                // Handle incoming analytics events
                Some(msg) = self.receiver.recv() => {
                    match msg {
                        ActorMessage::RecordEvent(event) => {
                            // Fast local append in Layer 1 buffer (no locks!)
                            self.buffer
                                .entry(event.short_code.clone())
                                .or_insert_with(Vec::new)
                                .push(event);
                        }
                        ActorMessage::Shutdown => {
                            info!("Analytics actor received shutdown signal, flushing...");
                            // Flush Layer 1 → Layer 2
                            self.flush_buffer_to_shared();
                            break;
                        }
                    }
                }
                // Fast flush: Layer 1 → Layer 2 (100ms default)
                _ = fast_flush_ticker.tick() => {
                    self.flush_buffer_to_shared();
                }
                // Channel closed without shutdown message
                else => {
                    warn!("Analytics actor channel closed unexpectedly, flushing...");
                    self.flush_buffer_to_shared();
                    break;
                }
            }
        }
    }

    /// Flush Layer 1 (local buffer) → Layer 2 (shared DashMap)
    /// This is fast and non-blocking
    fn flush_buffer_to_shared(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        for (short_code, events) in self.buffer.drain() {
            self.shared_buffer
                .entry(short_code)
                .and_modify(|existing| existing.extend(events.clone()))
                .or_insert(events);
        }
    }
}

/// In-memory analytics aggregator
pub struct AnalyticsAggregator {
    /// In-memory aggregation map (used when GeoIP service is available)
    aggregates: Arc<DashMap<AnalyticsKey, AnalyticsValue>>,
    
    /// Actor message sender for lock-free event recording
    actor_tx: mpsc::Sender<ActorMessage>,
    
    /// Shared event buffer (Layer 2) for concurrent flush access
    shared_buffer: Arc<DashMap<String, Vec<AnalyticsEvent>>>,
    
    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
}

impl AnalyticsAggregator {
    /// Create a new analytics aggregator with configurable parameters
    pub fn new_with_config(
        buffer_size: usize,
        fast_flush_interval_ms: u64,
    ) -> Self {
        let (actor_tx, actor_rx) = mpsc::channel(buffer_size);
        let shutdown = Arc::new(Mutex::new(false));
        let shared_buffer = Arc::new(DashMap::new());
        
        // Spawn the analytics actor
        let actor = AnalyticsActor {
            receiver: actor_rx,
            buffer: HashMap::new(),
            shared_buffer: Arc::clone(&shared_buffer),
            fast_flush_interval: Duration::from_millis(fast_flush_interval_ms),
        };
        
        tokio::spawn(async move {
            actor.run().await;
        });
        
        Self {
            aggregates: Arc::new(DashMap::new()),
            actor_tx,
            shared_buffer,
            shutdown,
        }
    }
    
    /// Create a new analytics aggregator with default settings
    pub fn new() -> Self {
        Self::new_with_config(
            100_000, // 100k event buffer
            100,     // 100ms fast flush interval
        )
    }

    /// Record a visit event (lightweight - defers GeoIP lookup)
    ///
    /// This is the HOT PATH method called on every request.
    /// Uses lock-free mpsc channel to avoid contention on hot keys.
    /// The GeoIP lookups are deferred until flush time.
    pub fn record_event(&self, event: AnalyticsEvent) {
        // Send to actor channel (lock-free, non-blocking)
        // If channel is full, log warning and drop event
        if let Err(_) = self.actor_tx.try_send(ActorMessage::RecordEvent(event)) {
            warn!("Analytics event buffer full, dropping event");
        }
    }

    /// Record a visit event (legacy - with GeoIP lookup already done)
    ///
    /// This increments the counter for the aggregated analytics key
    /// derived from the record.
    pub fn record(&self, record: AnalyticsRecord) {
        let key = AnalyticsKey::from_record(&record);
        
        self.aggregates
            .entry(key)
            .and_modify(|v| v.count += 1)
            .or_insert_with(|| AnalyticsValue { count: 1 });
    }

    /// Drain all aggregated analytics and return them
    ///
    /// This clears the in-memory aggregates and returns them for
    /// batch flushing to the database.
    pub fn drain(&self) -> Vec<(AnalyticsKey, AnalyticsValue)> {
        let mut result = Vec::new();
        
        // Collect all keys
        let keys: Vec<AnalyticsKey> = self.aggregates.iter()
            .map(|entry| entry.key().clone())
            .collect();
        
        // Remove and collect values
        for key in keys {
            if let Some((_, value)) = self.aggregates.remove(&key) {
                result.push((key, value));
            }
        }
        
        result
    }
    
    /// Drain event buffer and return all events
    ///
    /// This clears the shared event buffer and returns all events for
    /// processing with GeoIP lookups.
    pub fn drain_events(&self) -> Vec<AnalyticsEvent> {
        let mut result = Vec::new();
        
        // Collect all keys from shared buffer (Layer 2)
        let keys: Vec<String> = self.shared_buffer.iter()
            .map(|entry| entry.key().clone())
            .collect();
        
        // Remove and collect events
        for key in keys {
            if let Some((_, mut events)) = self.shared_buffer.remove(&key) {
                result.append(&mut events);
            }
        }
        
        result
    }

    /// Start the background flush task with storage callback
    ///
    /// This spawns a tokio task that periodically drains aggregates
    /// and flushes them to the database.
    pub fn start_flush_task_with_storage<F>(
        &self,
        flush_interval_secs: u64,
        flush_fn: F,
    ) -> tokio::task::JoinHandle<()>
    where
        F: Fn(Vec<(AnalyticsKey, AnalyticsValue)>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + 'static,
    {
        let aggregates = Arc::clone(&self.aggregates);
        let shutdown = Arc::clone(&self.shutdown);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(flush_interval_secs)
            );
            
            loop {
                interval.tick().await;
                
                // Check shutdown signal
                if *shutdown.lock().await {
                    info!("Analytics aggregator flush task shutting down");
                    break;
                }
                
                // Drain aggregates
                let count = aggregates.len();
                if count > 0 {
                    debug!("Draining {} analytics aggregates", count);
                    
                    // Collect keys and values
                    let mut entries = Vec::new();
                    let keys: Vec<AnalyticsKey> = aggregates.iter()
                        .map(|entry| entry.key().clone())
                        .collect();
                    
                    for key in keys {
                        if let Some((k, v)) = aggregates.remove(&key) {
                            entries.push((k, v));
                        }
                    }
                    
                    // Call flush function
                    if !entries.is_empty() {
                        flush_fn(entries).await;
                    }
                }
            }
        })
    }

    /// Start the background flush task (without storage - for backward compatibility)
    ///
    /// This spawns a tokio task that periodically drains aggregates.
    pub fn start_flush_task(
        &self,
        flush_interval_secs: u64,
    ) -> tokio::task::JoinHandle<()> {
        let aggregates = Arc::clone(&self.aggregates);
        let shutdown = Arc::clone(&self.shutdown);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(flush_interval_secs)
            );
            
            loop {
                interval.tick().await;
                
                // Check shutdown signal
                if *shutdown.lock().await {
                    info!("Analytics aggregator flush task shutting down");
                    break;
                }
                
                // Drain and log aggregates
                let count = aggregates.len();
                if count > 0 {
                    debug!("Draining {} analytics aggregates", count);
                    
                    // Collect keys
                    let keys: Vec<AnalyticsKey> = aggregates.iter()
                        .map(|entry| entry.key().clone())
                        .collect();
                    
                    // Remove entries
                    for key in keys {
                        aggregates.remove(&key);
                    }
                    
                    // TODO: Batch insert into database
                    // For now, we just drain to prevent unbounded memory growth
                }
            }
        })
    }
    
    /// Start the background flush task with GeoIP service (OPTIMIZED)
    ///
    /// This spawns a tokio task that periodically drains events,
    /// performs GeoIP lookups in batch, aggregates, and flushes to database.
    /// This is the OPTIMIZED path that keeps GeoIP lookups off the hot path.
    pub fn start_flush_task_with_geoip<F>(
        &self,
        flush_interval_secs: u64,
        geoip_service: Arc<crate::analytics::GeoIpService>,
        flush_fn: F,
    ) -> tokio::task::JoinHandle<()>
    where
        F: Fn(Vec<(AnalyticsKey, AnalyticsValue)>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + 'static,
    {
        let shared_buffer = Arc::clone(&self.shared_buffer);
        let aggregates = Arc::clone(&self.aggregates);
        let shutdown = Arc::clone(&self.shutdown);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(flush_interval_secs)
            );
            
            loop {
                interval.tick().await;
                
                // Check shutdown signal
                if *shutdown.lock().await {
                    info!("Analytics aggregator flush task shutting down");
                    break;
                }
                
                // Drain events from shared buffer (Layer 2) and process with GeoIP
                let event_count = shared_buffer.len();
                if event_count > 0 {
                    debug!("Processing {} analytics event buffers", event_count);
                    
                    // Collect all event buffers from shared buffer
                    let keys: Vec<String> = shared_buffer.iter()
                        .map(|entry| entry.key().clone())
                        .collect();
                    
                    // Process each buffer
                    for key in keys {
                        if let Some((_, events)) = shared_buffer.remove(&key) {
                            // Process events with GeoIP lookup (off hot path)
                            for event in events {
                                let geo_location = geoip_service.lookup(event.client_ip);
                                let analytics_key = AnalyticsKey::from_event(&event, &geo_location);
                                
                                // Aggregate the result
                                aggregates
                                    .entry(analytics_key)
                                    .and_modify(|v| v.count += 1)
                                    .or_insert_with(|| AnalyticsValue { count: 1 });
                            }
                        }
                    }
                }
                
                // Now drain and flush aggregates
                let agg_count = aggregates.len();
                if agg_count > 0 {
                    debug!("Flushing {} analytics aggregates", agg_count);
                    
                    let mut entries = Vec::new();
                    let keys: Vec<AnalyticsKey> = aggregates.iter()
                        .map(|entry| entry.key().clone())
                        .collect();
                    
                    for key in keys {
                        if let Some((k, v)) = aggregates.remove(&key) {
                            entries.push((k, v));
                        }
                    }
                    
                    // Call flush function
                    if !entries.is_empty() {
                        flush_fn(entries).await;
                    }
                }
            }
        })
    }

    /// Get aggregated analytics from in-memory data for a specific short code
    /// This is used for near real-time analytics display
    /// Returns aggregates grouped by the specified dimension
    pub fn get_in_memory_aggregate(
        &self,
        short_code: &str,
        group_by: &str,
    ) -> Vec<(String, i64)> {
        use std::collections::HashMap;
        
        let mut grouped: HashMap<String, i64> = HashMap::new();
        
        // Aggregate from both processed aggregates (Layer 3) and pending events (Layer 2)
        
        // Process from aggregates (already GeoIP resolved)
        for entry in self.aggregates.iter() {
            let key = entry.key();
            if key.short_code != short_code {
                continue;
            }
            
            let dimension = match group_by {
                "country" => key.country_code.clone().unwrap_or_else(|| "Unknown".to_string()),
                "region" => key.region.clone().unwrap_or_else(|| "Unknown".to_string()),
                "city" => key.city.clone().unwrap_or_else(|| "Unknown".to_string()),
                "asn" => key.asn.map(|a| a.to_string()).unwrap_or_else(|| "Unknown".to_string()),
                "hour" => key.time_bucket.to_string(),
                "day" => ((key.time_bucket / 86400) * 86400).to_string(),
                _ => key.country_code.clone().unwrap_or_else(|| "Unknown".to_string()),
            };
            
            *grouped.entry(dimension).or_insert(0) += entry.value().count as i64;
        }
        
        // Process from shared buffer (Layer 2) - events pending GeoIP lookup
        // These will be displayed as "Unknown" since GeoIP hasn't been resolved yet
        let unknown_count: i64 = self.shared_buffer
            .iter()
            .filter(|entry| entry.key() == short_code)
            .map(|entry| entry.value().len() as i64)
            .sum();
        
        if unknown_count > 0 {
            *grouped.entry("Unknown".to_string()).or_insert(0) += unknown_count;
        }
        
        // Convert to Vec and sort by count descending
        let mut result: Vec<(String, i64)> = grouped.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Signal shutdown to the flush task and actor
    pub async fn shutdown(&self) {
        // Send shutdown message to actor
        let _ = self.actor_tx.send(ActorMessage::Shutdown).await;
        
        // Signal shutdown to flush task
        let mut shutdown = self.shutdown.lock().await;
        *shutdown = true;
    }

    /// Get the current number of aggregated entries
    pub fn len(&self) -> usize {
        self.aggregates.len()
    }

    /// Check if the aggregator is empty
    pub fn is_empty(&self) -> bool {
        self.aggregates.is_empty()
    }
}

impl Default for AnalyticsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::models::GeoLocation;

    #[tokio::test]
    async fn test_aggregator_record() {
        let aggregator = AnalyticsAggregator::new();
        
        let record = AnalyticsRecord {
            short_code: "test123".to_string(),
            timestamp: 1234567890,
            geo_location: GeoLocation {
                country_code: Some("US".to_string()),
                country_name: Some("United States".to_string()),
                region: Some("CA".to_string()),
                city: Some("San Francisco".to_string()),
                asn: None,
                asn_org: None,
                ip_version: 4,
            },
            client_ip: None,
        };
        
        aggregator.record(record.clone());
        assert_eq!(aggregator.len(), 1);
        
        // Recording the same event should increment the counter
        aggregator.record(record);
        assert_eq!(aggregator.len(), 1);
    }

    #[tokio::test]
    async fn test_aggregator_drain() {
        let aggregator = AnalyticsAggregator::new();
        
        let record = AnalyticsRecord {
            short_code: "test123".to_string(),
            timestamp: 1234567890,
            geo_location: GeoLocation::default(),
            client_ip: None,
        };
        
        aggregator.record(record.clone());
        aggregator.record(record);
        
        let drained = aggregator.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].1.count, 2);
        assert_eq!(aggregator.len(), 0);
    }
}
