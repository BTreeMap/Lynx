//! Stress tests for analytics module
//!
//! These tests ensure the analytics system maintains data consistency and doesn't lose data
//! under various challenging conditions including concurrent operations, rapid flush cycles,
//! and edge cases.

use lynx::analytics::{AnalyticsAggregator, AnalyticsEvent, AnalyticsRecord, GeoLocation};
use lynx::storage::{SqliteStorage, Storage};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

/// Helper to create test storage
async fn create_test_storage() -> Arc<dyn Storage> {
    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    Arc::new(storage)
}

#[tokio::test]
async fn test_concurrent_event_recording() {
    // Test that concurrent event recording doesn't lose data
    let agg = Arc::new(AnalyticsAggregator::new());
    
    let mut handles = vec![];
    
    // Spawn 10 concurrent tasks that each record 100 events
    for task_id in 0..10 {
        let agg_clone = Arc::clone(&agg);
        let handle = tokio::spawn(async move {
            for i in 0..100 {
                let event = AnalyticsEvent {
                    short_code: format!("code{}", task_id % 3),
                    client_ip: "192.168.1.1".parse().unwrap(),
                    timestamp: 1000000 + i,
                };
                agg_clone.record_event(event);
            }
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Wait a bit for events to be processed
    sleep(Duration::from_millis(500)).await;
    
    // Drain events and verify count
    let events = agg.drain_events();
    assert_eq!(events.len(), 1000, "Should have all 1000 events");
}

#[tokio::test]
async fn test_rapid_flush_cycles() {
    // Test that rapid flush cycles don't corrupt data
    let storage = create_test_storage().await;
    let agg = Arc::new(AnalyticsAggregator::new());
    
    // Record initial data
    for i in 0..100 {
        let rec = AnalyticsRecord {
            short_code: "rapid".to_string(),
            timestamp: 1000000 + (i * 3600), // Different time buckets
            geo_location: GeoLocation {
                country_code: Some("US".to_string()),
                country_name: Some("United States".to_string()),
                region: Some(format!("Region{}", i % 5)),
                city: Some(format!("City{}", i % 10)),
                asn: Some(15169),
                asn_org: Some("Google".to_string()),
                ip_version: 4,
            },
            client_ip: None,
        };
        agg.record(rec);
    }
    
    // Perform rapid flushes
    for _ in 0..10 {
        let entries = agg.drain();
        let records: Vec<_> = entries
            .into_iter()
            .map(|(k, v)| {
                (
                    k.short_code,
                    k.time_bucket,
                    k.country_code,
                    k.region,
                    k.city,
                    k.asn.map(|a| a as i64),
                    k.ip_version as i32,
                    v.count as i64,
                )
            })
            .collect();
        
        if !records.is_empty() {
            storage.upsert_analytics_batch(records).await.unwrap();
        }
        
        // Record more data between flushes
        for i in 0..10 {
            let rec = AnalyticsRecord {
                short_code: "rapid".to_string(),
                timestamp: 2000000 + (i * 3600),
                geo_location: GeoLocation {
                    country_code: Some("CA".to_string()),
                    country_name: Some("Canada".to_string()),
                    region: Some("ON".to_string()),
                    city: Some("Toronto".to_string()),
                    asn: Some(16509),
                    asn_org: Some("Amazon".to_string()),
                    ip_version: 4,
                },
                client_ip: None,
            };
            agg.record(rec);
        }
    }
    
    // Final flush
    let entries = agg.drain();
    if !entries.is_empty() {
        let records: Vec<_> = entries
            .into_iter()
            .map(|(k, v)| {
                (
                    k.short_code,
                    k.time_bucket,
                    k.country_code,
                    k.region,
                    k.city,
                    k.asn.map(|a| a as i64),
                    k.ip_version as i32,
                    v.count as i64,
                )
            })
            .collect();
        storage.upsert_analytics_batch(records).await.unwrap();
    }
    
    // Verify all data was saved
    let analytics = storage
        .get_analytics("rapid", None, None, 1000)
        .await
        .unwrap();
    
    let total: i64 = analytics.iter().map(|a| a.visit_count).sum();
    assert_eq!(total, 200, "Should have all 200 visits (100 initial + 100 incremental)");
}

#[tokio::test]
async fn test_prune_preserves_data_consistency() {
    // Test that pruning maintains data consistency and doesn't lose visit counts
    let storage = create_test_storage().await;
    
    // Create a short code
    storage
        .create_with_code("prune_test", "https://example.com", Some("user1"))
        .await
        .unwrap();
    
    // Insert old data with various dimensions
    let old_time = chrono::Utc::now().timestamp() - (60 * 86400); // 60 days ago
    let mut records = vec![];
    
    // Create 100 entries with different dimensions
    for i in 0..100 {
        records.push((
            "prune_test".to_string(),
            old_time + (i * 3600), // Different hours
            Some(format!("C{}", i % 5)),  // 5 different countries
            Some(format!("R{}", i % 10)), // 10 different regions
            Some(format!("City{}", i % 20)), // 20 different cities
            Some(15169 + (i % 3) as i64), // 3 different ASNs
            4,
            1,
        ));
    }
    
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Verify we have 100 entries
    let before_prune = storage
        .get_analytics("prune_test", None, None, 200)
        .await
        .unwrap();
    assert_eq!(before_prune.len(), 100);
    let total_before: i64 = before_prune.iter().map(|a| a.visit_count).sum();
    assert_eq!(total_before, 100);
    
    // Prune with dropping city and region
    let (deleted, inserted) = storage
        .prune_analytics(30, &vec!["city".to_string(), "region".to_string()])
        .await
        .unwrap();
    
    assert_eq!(deleted, 100, "Should have deleted all old entries");
    assert!(inserted > 0, "Should have created aggregated entries");
    
    // Verify all visits are preserved
    let after_prune = storage
        .get_analytics("prune_test", None, None, 200)
        .await
        .unwrap();
    let total_after: i64 = after_prune.iter().map(|a| a.visit_count).sum();
    assert_eq!(total_after, 100, "Total visit count should be preserved");
    
    // Verify dimensions were dropped
    let has_dropped_city = after_prune
        .iter()
        .any(|a| a.city == Some("<dropped>".to_string()));
    assert!(has_dropped_city, "Should have <dropped> marker for city");
}

#[tokio::test]
async fn test_aggregation_consistency_with_multiple_upserts() {
    // Test that multiple upserts maintain consistency
    let storage = create_test_storage().await;
    
    storage
        .create_with_code("multi_upsert", "https://example.com", Some("user1"))
        .await
        .unwrap();
    
    let time_bucket = 1698768000;
    
    // First batch
    let records1 = vec![(
        "multi_upsert".to_string(),
        time_bucket,
        Some("US".to_string()),
        Some("CA".to_string()),
        Some("SF".to_string()),
        Some(15169),
        4,
        10,
    )];
    storage.upsert_analytics_batch(records1).await.unwrap();
    
    // Second batch (same key, should increment)
    let records2 = vec![(
        "multi_upsert".to_string(),
        time_bucket,
        Some("US".to_string()),
        Some("CA".to_string()),
        Some("SF".to_string()),
        Some(15169),
        4,
        5,
    )];
    storage.upsert_analytics_batch(records2).await.unwrap();
    
    // Third batch (same key, should increment again)
    let records3 = vec![(
        "multi_upsert".to_string(),
        time_bucket,
        Some("US".to_string()),
        Some("CA".to_string()),
        Some("SF".to_string()),
        Some(15169),
        4,
        3,
    )];
    storage.upsert_analytics_batch(records3).await.unwrap();
    
    // Verify total is correct
    let analytics = storage
        .get_analytics("multi_upsert", None, None, 100)
        .await
        .unwrap();
    
    assert_eq!(analytics.len(), 1, "Should have exactly one entry");
    assert_eq!(analytics[0].visit_count, 18, "Should have 10+5+3=18 visits");
}

#[tokio::test]
async fn test_time_range_filtering_edge_cases() {
    // Test edge cases in time range filtering
    let storage = create_test_storage().await;
    
    storage
        .create_with_code("time_test", "https://example.com", Some("user1"))
        .await
        .unwrap();
    
    // Insert records at specific timestamps
    let records = vec![
        (
            "time_test".to_string(),
            1000,
            Some("US".to_string()),
            None,
            None,
            None,
            4,
            1,
        ),
        (
            "time_test".to_string(),
            2000,
            Some("US".to_string()),
            None,
            None,
            None,
            4,
            2,
        ),
        (
            "time_test".to_string(),
            3000,
            Some("US".to_string()),
            None,
            None,
            None,
            4,
            3,
        ),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Test exact boundary: start_time = 2000
    let result = storage
        .get_analytics("time_test", Some(2000), None, 100)
        .await
        .unwrap();
    let total: i64 = result.iter().map(|a| a.visit_count).sum();
    assert_eq!(total, 5, "Should include entries at and after 2000");
    
    // Test exact boundary: end_time = 2000
    let result = storage
        .get_analytics("time_test", None, Some(2000), 100)
        .await
        .unwrap();
    let total: i64 = result.iter().map(|a| a.visit_count).sum();
    assert_eq!(total, 3, "Should include entries at and before 2000");
    
    // Test exact match
    let result = storage
        .get_analytics("time_test", Some(2000), Some(2000), 100)
        .await
        .unwrap();
    let total: i64 = result.iter().map(|a| a.visit_count).sum();
    assert_eq!(total, 2, "Should include only entry at 2000");
}

#[tokio::test]
async fn test_aggregation_with_null_dimensions() {
    // Test that aggregation correctly handles NULL dimensions
    let storage = create_test_storage().await;
    
    storage
        .create_with_code("null_test", "https://example.com", Some("user1"))
        .await
        .unwrap();
    
    let time_bucket = 1698768000;
    let records = vec![
        // Entry with all dimensions
        (
            "null_test".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("CA".to_string()),
            Some("SF".to_string()),
            Some(15169),
            4,
            5,
        ),
        // Entry with NULL region and city
        (
            "null_test".to_string(),
            time_bucket,
            Some("US".to_string()),
            None,
            None,
            Some(15169),
            4,
            3,
        ),
        // Entry with NULL ASN
        (
            "null_test".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("CA".to_string()),
            Some("LA".to_string()),
            None,
            4,
            2,
        ),
    ];
    
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Aggregate by country (should combine all)
    let country_agg = storage
        .get_analytics_aggregate("null_test", None, None, "country", 10)
        .await
        .unwrap();
    assert_eq!(country_agg.len(), 1);
    assert_eq!(country_agg[0].visit_count, 10);
    
    // Aggregate by region (should handle NULLs correctly)
    let region_agg = storage
        .get_analytics_aggregate("null_test", None, None, "region", 10)
        .await
        .unwrap();
    assert!(region_agg.len() >= 2, "Should have at least 2 regions (CA and Unknown)");
    
    // Aggregate by ASN (should handle NULLs)
    let asn_agg = storage
        .get_analytics_aggregate("null_test", None, None, "asn", 10)
        .await
        .unwrap();
    assert!(asn_agg.len() >= 1, "Should have at least 1 ASN entry");
}
