//! In-memory analytics aggregator with periodic flush
//!
//! This module provides high-performance in-memory aggregation of
//! analytics records that are periodically flushed to the database.
//!
//! PERFORMANCE OPTIMIZATION: GeoIP lookups are deferred until flush time
//! to keep the hot path (request handling) as fast as possible.

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::analytics::models::{AnalyticsKey, AnalyticsEvent, AnalyticsRecord, AnalyticsValue};

/// In-memory analytics aggregator
pub struct AnalyticsAggregator {
    /// In-memory aggregation map (used when GeoIP service is available)
    aggregates: Arc<DashMap<AnalyticsKey, AnalyticsValue>>,
    
    /// Lightweight event buffer for deferred GeoIP lookup
    /// This is used to defer expensive GeoIP lookups off the hot path
    event_buffer: Arc<DashMap<String, Vec<AnalyticsEvent>>>,
    
    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
}

impl AnalyticsAggregator {
    /// Create a new analytics aggregator
    pub fn new() -> Self {
        Self {
            aggregates: Arc::new(DashMap::new()),
            event_buffer: Arc::new(DashMap::new()),
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    /// Record a visit event (lightweight - defers GeoIP lookup)
    ///
    /// This is the HOT PATH method called on every request.
    /// It stores a lightweight event without doing expensive GeoIP lookups.
    /// The GeoIP lookups are deferred until flush time.
    pub fn record_event(&self, event: AnalyticsEvent) {
        // Use the short_code as the buffer key for efficient batching
        let short_code = event.short_code.clone();
        
        self.event_buffer
            .entry(short_code)
            .and_modify(|events| events.push(event.clone()))
            .or_insert_with(|| vec![event]);
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
    /// This clears the event buffer and returns all events for
    /// processing with GeoIP lookups.
    pub fn drain_events(&self) -> Vec<AnalyticsEvent> {
        let mut result = Vec::new();
        
        // Collect all keys
        let keys: Vec<String> = self.event_buffer.iter()
            .map(|entry| entry.key().clone())
            .collect();
        
        // Remove and collect events
        for key in keys {
            if let Some((_, mut events)) = self.event_buffer.remove(&key) {
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
        let event_buffer = Arc::clone(&self.event_buffer);
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
                
                // Drain events and process them with GeoIP lookups
                let event_count = event_buffer.len();
                if event_count > 0 {
                    debug!("Processing {} analytics event buffers", event_count);
                    
                    // Collect all event buffers
                    let keys: Vec<String> = event_buffer.iter()
                        .map(|entry| entry.key().clone())
                        .collect();
                    
                    // Process each buffer
                    for key in keys {
                        if let Some((_, events)) = event_buffer.remove(&key) {
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

    /// Signal shutdown to the flush task
    pub async fn shutdown(&self) {
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

    #[test]
    fn test_aggregator_record() {
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

    #[test]
    fn test_aggregator_drain() {
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
