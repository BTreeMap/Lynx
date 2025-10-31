//! Integration tests for analytics prune and align commands

use lynx::analytics::{DROPPED_DIMENSION_MARKER, ALIGNMENT_TIME_BUCKET};
use lynx::storage::{Storage, SqliteStorage};
use std::sync::Arc;

async fn setup_test_storage() -> Arc<SqliteStorage> {
    let storage = SqliteStorage::new(":memory:", 1).await.unwrap();
    storage.init().await.unwrap();
    Arc::new(storage)
}

#[tokio::test]
async fn test_prune_with_time_bucket_drop() {
    let storage = setup_test_storage().await;
    
    // Create test URL
    storage.create_with_code("test1", "https://example.com", Some("user1")).await.unwrap();
    
    // Insert analytics with different time buckets (all older than 30 days)
    let old_time = chrono::Utc::now().timestamp() - (40 * 86400);
    let records = vec![
        ("test1".to_string(), old_time, Some("US".to_string()), Some("CA".to_string()), Some("SF".to_string()), None, 4, 10),
        ("test1".to_string(), old_time + 3600, Some("US".to_string()), Some("CA".to_string()), Some("SF".to_string()), None, 4, 5),
        ("test1".to_string(), old_time + 7200, Some("US".to_string()), Some("NY".to_string()), Some("NYC".to_string()), None, 4, 3),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Prune analytics, dropping time_bucket
    let (deleted, inserted) = storage.prune_analytics(30, &vec!["time_bucket".to_string()]).await.unwrap();
    
    assert_eq!(deleted, 3, "Should delete 3 old entries");
    assert!(inserted >= 1, "Should insert at least 1 aggregated entry");
    
    // Verify total count is preserved
    let analytics = storage.get_analytics("test1", None, None, 100).await.unwrap();
    let total: i64 = analytics.iter().map(|a| a.visit_count).sum();
    assert_eq!(total, 18, "Total count should be preserved");
}

#[tokio::test]
async fn test_prune_with_geo_dimensions_drop() {
    let storage = setup_test_storage().await;
    
    // Create test URL
    storage.create_with_code("test2", "https://example.com", Some("user1")).await.unwrap();
    
    // Insert analytics with different cities
    let old_time = chrono::Utc::now().timestamp() - (40 * 86400);
    let records = vec![
        ("test2".to_string(), old_time, Some("US".to_string()), Some("CA".to_string()), Some("SF".to_string()), None, 4, 10),
        ("test2".to_string(), old_time, Some("US".to_string()), Some("CA".to_string()), Some("LA".to_string()), None, 4, 5),
        ("test2".to_string(), old_time, Some("US".to_string()), Some("NY".to_string()), Some("NYC".to_string()), None, 4, 3),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Prune analytics, dropping city and region
    let (deleted, inserted) = storage.prune_analytics(30, &vec!["city".to_string(), "region".to_string()]).await.unwrap();
    
    assert_eq!(deleted, 3, "Should delete 3 old entries");
    assert!(inserted >= 1, "Should insert aggregated entries");
    
    // Verify total count is preserved
    let analytics = storage.get_analytics("test2", None, None, 100).await.unwrap();
    let total: i64 = analytics.iter().map(|a| a.visit_count).sum();
    assert_eq!(total, 18, "Total count should be preserved");
    
    // Verify dropped markers are present
    let has_dropped = analytics.iter().any(|a| 
        a.city == Some(DROPPED_DIMENSION_MARKER.to_string()) &&
        a.region == Some(DROPPED_DIMENSION_MARKER.to_string())
    );
    assert!(has_dropped, "Should have entries with dropped markers");
}

#[tokio::test]
async fn test_prune_preserves_alignment_entries() {
    let storage = setup_test_storage().await;
    
    // Create test URL
    storage.create_with_code("test3", "https://example.com", Some("user1")).await.unwrap();
    
    // Add clicks before analytics
    storage.increment_clicks("test3", 100).await.unwrap();
    
    // Create alignment entry
    storage.align_analytics_with_clicks("test3").await.unwrap();
    
    // Add old analytics
    let old_time = chrono::Utc::now().timestamp() - (40 * 86400);
    let records = vec![
        ("test3".to_string(), old_time, Some("US".to_string()), Some("CA".to_string()), Some("SF".to_string()), None, 4, 10),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Prune analytics
    let (deleted, _inserted) = storage.prune_analytics(30, &vec!["time_bucket".to_string()]).await.unwrap();
    
    assert_eq!(deleted, 1, "Should only delete the old entry, not the alignment entry");
    
    // Verify alignment entry still exists
    let analytics = storage.get_analytics("test3", None, None, 100).await.unwrap();
    let has_alignment = analytics.iter().any(|a| a.time_bucket == ALIGNMENT_TIME_BUCKET);
    assert!(has_alignment, "Alignment entry should still exist after pruning");
    
    // Total should still be 110 (100 from alignment + 10 from pruned entry)
    let total: i64 = analytics.iter().map(|a| a.visit_count).sum();
    assert_eq!(total, 110, "Total count should include alignment entry");
}

#[tokio::test]
async fn test_align_single_short_code() {
    let storage = setup_test_storage().await;
    
    // Create test URL with clicks
    storage.create_with_code("align1", "https://example.com", Some("user1")).await.unwrap();
    storage.increment_clicks("align1", 50).await.unwrap();
    
    // Add some analytics (less than clicks)
    let time_bucket = chrono::Utc::now().timestamp();
    let records = vec![
        ("align1".to_string(), time_bucket, Some("US".to_string()), Some("CA".to_string()), Some("SF".to_string()), None, 4, 20),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Check difference before alignment
    let (clicks, analytics_count, diff) = storage.get_analytics_click_difference("align1").await.unwrap();
    assert_eq!(clicks, 50);
    assert_eq!(analytics_count, 20);
    assert_eq!(diff, 30);
    
    // Align
    let inserted = storage.align_analytics_with_clicks("align1").await.unwrap();
    assert_eq!(inserted, 1, "Should insert 1 alignment entry");
    
    // Verify alignment worked
    let (clicks_after, analytics_after, diff_after) = storage.get_analytics_click_difference("align1").await.unwrap();
    assert_eq!(clicks_after, 50);
    assert_eq!(analytics_after, 50);
    assert_eq!(diff_after, 0);
}

#[tokio::test]
async fn test_align_all_misaligned_codes() {
    let storage = setup_test_storage().await;
    
    // Create multiple URLs with different alignment states
    storage.create_with_code("aligned", "https://example.com", Some("user1")).await.unwrap();
    storage.increment_clicks("aligned", 10).await.unwrap();
    
    storage.create_with_code("misaligned1", "https://example.com", Some("user1")).await.unwrap();
    storage.increment_clicks("misaligned1", 100).await.unwrap();
    
    storage.create_with_code("misaligned2", "https://example.com", Some("user1")).await.unwrap();
    storage.increment_clicks("misaligned2", 50).await.unwrap();
    
    // Add analytics for aligned code (perfect match)
    let time_bucket = chrono::Utc::now().timestamp();
    let records = vec![
        ("aligned".to_string(), time_bucket, Some("US".to_string()), None, None, None, 4, 10),
        ("misaligned1".to_string(), time_bucket, Some("US".to_string()), None, None, None, 4, 30),
        ("misaligned2".to_string(), time_bucket, Some("US".to_string()), None, None, None, 4, 10),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Get all misaligned
    let misaligned = storage.get_all_misaligned_analytics().await.unwrap();
    
    assert_eq!(misaligned.len(), 2, "Should have 2 misaligned codes");
    
    // Verify the misaligned codes
    let codes: Vec<String> = misaligned.iter().map(|(code, _, _, _)| code.clone()).collect();
    assert!(codes.contains(&"misaligned1".to_string()));
    assert!(codes.contains(&"misaligned2".to_string()));
    assert!(!codes.contains(&"aligned".to_string()));
    
    // Align all
    for (code, _, _, _) in &misaligned {
        storage.align_analytics_with_clicks(code).await.unwrap();
    }
    
    // Verify all are now aligned
    let misaligned_after = storage.get_all_misaligned_analytics().await.unwrap();
    assert_eq!(misaligned_after.len(), 0, "All codes should be aligned now");
}

#[tokio::test]
async fn test_align_no_action_when_already_aligned() {
    let storage = setup_test_storage().await;
    
    // Create URL with matching clicks and analytics
    storage.create_with_code("perfect", "https://example.com", Some("user1")).await.unwrap();
    storage.increment_clicks("perfect", 25).await.unwrap();
    
    let time_bucket = chrono::Utc::now().timestamp();
    let records = vec![
        ("perfect".to_string(), time_bucket, Some("US".to_string()), None, None, None, 4, 25),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Try to align
    let inserted = storage.align_analytics_with_clicks("perfect").await.unwrap();
    assert_eq!(inserted, 0, "Should not insert anything when already aligned");
}

#[tokio::test]
async fn test_alignment_entry_has_correct_markers() {
    let storage = setup_test_storage().await;
    
    // Create URL with clicks
    storage.create_with_code("markers", "https://example.com", Some("user1")).await.unwrap();
    storage.increment_clicks("markers", 100).await.unwrap();
    
    // Align (no analytics yet)
    storage.align_analytics_with_clicks("markers").await.unwrap();
    
    // Check the alignment entry
    let analytics = storage.get_analytics("markers", None, None, 100).await.unwrap();
    assert_eq!(analytics.len(), 1);
    
    let entry = &analytics[0];
    assert_eq!(entry.time_bucket, ALIGNMENT_TIME_BUCKET);
    assert_eq!(entry.country_code, Some(DROPPED_DIMENSION_MARKER.to_string()));
    assert_eq!(entry.region, Some(DROPPED_DIMENSION_MARKER.to_string()));
    assert_eq!(entry.city, Some(DROPPED_DIMENSION_MARKER.to_string()));
    assert_eq!(entry.asn, None);
    assert_eq!(entry.visit_count, 100);
}

#[tokio::test]
async fn test_prune_and_align_together() {
    let storage = setup_test_storage().await;
    
    // Create URL with old analytics and missing recent clicks
    storage.create_with_code("combined", "https://example.com", Some("user1")).await.unwrap();
    storage.increment_clicks("combined", 200).await.unwrap(); // 200 clicks total
    
    // Add old analytics (100 clicks worth)
    let old_time = chrono::Utc::now().timestamp() - (40 * 86400);
    let records = vec![
        ("combined".to_string(), old_time, Some("US".to_string()), Some("CA".to_string()), Some("SF".to_string()), None, 4, 100),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();
    
    // Prune first (aggregates old data)
    let (deleted, _) = storage.prune_analytics(30, &vec!["city".to_string()]).await.unwrap();
    assert_eq!(deleted, 1);
    
    // Now align to account for missing clicks (200 total - 100 analytics = 100 missing)
    let inserted = storage.align_analytics_with_clicks("combined").await.unwrap();
    assert_eq!(inserted, 1);
    
    // Verify total
    let (clicks, analytics_count, diff) = storage.get_analytics_click_difference("combined").await.unwrap();
    assert_eq!(clicks, 200);
    assert_eq!(analytics_count, 200);
    assert_eq!(diff, 0);
}
