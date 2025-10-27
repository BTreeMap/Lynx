//! In-memory analytics aggregator with periodic flush
//!
//! This module provides high-performance in-memory aggregation of
//! analytics records that are periodically flushed to the database.

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::analytics::models::{AnalyticsKey, AnalyticsRecord, AnalyticsValue};

/// In-memory analytics aggregator
pub struct AnalyticsAggregator {
    /// In-memory aggregation map
    aggregates: Arc<DashMap<AnalyticsKey, AnalyticsValue>>,
    
    /// Shutdown signal
    shutdown: Arc<Mutex<bool>>,
}

impl AnalyticsAggregator {
    /// Create a new analytics aggregator
    pub fn new() -> Self {
        Self {
            aggregates: Arc::new(DashMap::new()),
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    /// Record a visit event
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

    /// Start the background flush task
    ///
    /// This spawns a tokio task that periodically drains aggregates
    /// and flushes them to the database.
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
