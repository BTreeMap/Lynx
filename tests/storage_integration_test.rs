//! Integration tests for storage module features
//!
//! These tests ensure comprehensive coverage of storage functionality including
//! user management, link operations, and admin features.

use lynx::storage::{SqliteStorage, Storage};
use std::sync::Arc;

/// Helper to create test storage
async fn create_test_storage() -> Arc<dyn Storage> {
    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    Arc::new(storage)
}

#[tokio::test]
async fn test_concurrent_url_creation() {
    // Test that concurrent URL creation handles conflicts correctly
    let storage = create_test_storage().await;
    
    let mut handles = vec![];
    
    // Try to create the same URL concurrently
    for i in 0..10 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            storage_clone
                .create_with_code("same_code", "https://example.com", Some(&format!("user{}", i)))
                .await
        });
        handles.push(handle);
    }
    
    // Exactly one should succeed, others should get Conflict error
    let mut success_count = 0;
    let mut conflict_count = 0;
    
    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => success_count += 1,
            Err(e) => {
                if matches!(e, lynx::storage::StorageError::Conflict) {
                    conflict_count += 1;
                } else {
                    panic!("Unexpected error: {:?}", e);
                }
            }
        }
    }
    
    assert_eq!(success_count, 1, "Exactly one creation should succeed");
    assert_eq!(conflict_count, 9, "All others should get conflict");
}

#[tokio::test]
async fn test_user_management_lifecycle() {
    // Test complete user lifecycle: create, update, promote, demote
    let storage = create_test_storage().await;
    
    // Create user
    storage
        .upsert_user("user123", Some("user@example.com"), "oauth")
        .await
        .unwrap();
    
    // List users
    let users = storage.list_all_users(10, 0).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].0, "user123");
    assert_eq!(users[0].2, "user@example.com");
    
    // Update user email
    storage
        .upsert_user("user123", Some("newemail@example.com"), "oauth")
        .await
        .unwrap();
    
    let users = storage.list_all_users(10, 0).await.unwrap();
    assert_eq!(users[0].2, "newemail@example.com");
    
    // Promote to admin
    assert!(!storage.is_manual_admin("user123", "oauth").await.unwrap());
    storage.promote_to_admin("user123", "oauth").await.unwrap();
    assert!(storage.is_manual_admin("user123", "oauth").await.unwrap());
    
    // List admins
    let admins = storage.list_manual_admins().await.unwrap();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins[0].0, "user123");
    
    // Demote from admin
    let demoted = storage.demote_from_admin("user123", "oauth").await.unwrap();
    assert!(demoted);
    assert!(!storage.is_manual_admin("user123", "oauth").await.unwrap());
    
    // Try to demote again (should return false)
    let demoted_again = storage.demote_from_admin("user123", "oauth").await.unwrap();
    assert!(!demoted_again);
}

#[tokio::test]
async fn test_bulk_link_operations() {
    // Test bulk deactivate and reactivate
    let storage = create_test_storage().await;
    
    // Create multiple links for a user
    for i in 1..=5 {
        storage
            .create_with_code(&format!("link{}", i), "https://example.com", Some("user1"))
            .await
            .unwrap();
    }
    
    // Create links for another user
    for i in 6..=8 {
        storage
            .create_with_code(&format!("link{}", i), "https://example.com", Some("user2"))
            .await
            .unwrap();
    }
    
    // Verify all are active
    for i in 1..=8 {
        let url = storage.get_authoritative(&format!("link{}", i)).await.unwrap();
        assert!(url.unwrap().is_active);
    }
    
    // Bulk deactivate user1's links
    let count = storage.bulk_deactivate_user_links("user1").await.unwrap();
    assert_eq!(count, 5);
    
    // Verify user1's links are deactivated
    for i in 1..=5 {
        let url = storage.get_authoritative(&format!("link{}", i)).await.unwrap();
        assert!(!url.unwrap().is_active);
    }
    
    // Verify user2's links are still active
    for i in 6..=8 {
        let url = storage.get_authoritative(&format!("link{}", i)).await.unwrap();
        assert!(url.unwrap().is_active);
    }
    
    // Bulk reactivate user1's links
    let count = storage.bulk_reactivate_user_links("user1").await.unwrap();
    assert_eq!(count, 5);
    
    // Verify all are active again
    for i in 1..=8 {
        let url = storage.get_authoritative(&format!("link{}", i)).await.unwrap();
        assert!(url.unwrap().is_active);
    }
}

#[tokio::test]
async fn test_cursor_pagination() {
    // Test cursor-based pagination for listing URLs
    let storage = create_test_storage().await;
    
    // Create 10 links with known timestamps
    for i in 0..10 {
        storage
            .create_with_code(&format!("page{}", i), "https://example.com", Some("user1"))
            .await
            .unwrap();
        
        // Sleep briefly to ensure different created_at times
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    // Get first page (limit 3)
    let page1 = storage.list_with_cursor(3, None, true, None).await.unwrap();
    assert_eq!(page1.len(), 3);
    
    // Get second page using cursor from last item of page1
    let last = page1.last().unwrap();
    let cursor = (last.created_at, last.id);
    let page2 = storage.list_with_cursor(3, Some(cursor), true, None).await.unwrap();
    assert_eq!(page2.len(), 3);
    
    // Verify pages don't overlap
    let page1_codes: Vec<String> = page1.iter().map(|u| u.short_code.clone()).collect();
    let page2_codes: Vec<String> = page2.iter().map(|u| u.short_code.clone()).collect();
    
    for code in &page2_codes {
        assert!(!page1_codes.contains(code), "Pages should not overlap");
    }
    
    // Continue paginating until we get all items
    let mut all_codes = page1_codes.clone();
    all_codes.extend(page2_codes.clone());
    
    let mut cursor = Some((page2.last().unwrap().created_at, page2.last().unwrap().id));
    
    while all_codes.len() < 10 {
        let page = storage.list_with_cursor(3, cursor, true, None).await.unwrap();
        if page.is_empty() {
            break;
        }
        let page_codes: Vec<String> = page.iter().map(|u| u.short_code.clone()).collect();
        all_codes.extend(page_codes);
        cursor = page.last().map(|u| (u.created_at, u.id));
    }
    
    assert_eq!(all_codes.len(), 10, "Should paginate through all items");
}

#[tokio::test]
async fn test_user_link_isolation() {
    // Test that users can only see their own links
    let storage = create_test_storage().await;
    
    // Create links for different users
    storage.create_with_code("user1_link1", "https://example.com", Some("user1")).await.unwrap();
    storage.create_with_code("user1_link2", "https://example.com", Some("user1")).await.unwrap();
    storage.create_with_code("user2_link1", "https://example.com", Some("user2")).await.unwrap();
    
    // User 1 should see only their links
    let user1_links = storage.list_with_cursor(10, None, false, Some("user1")).await.unwrap();
    assert_eq!(user1_links.len(), 2);
    for link in &user1_links {
        assert_eq!(link.created_by, Some("user1".to_string()));
    }
    
    // User 2 should see only their link
    let user2_links = storage.list_with_cursor(10, None, false, Some("user2")).await.unwrap();
    assert_eq!(user2_links.len(), 1);
    assert_eq!(user2_links[0].created_by, Some("user2".to_string()));
    
    // Admin should see all links
    let admin_links = storage.list_with_cursor(10, None, true, Some("admin")).await.unwrap();
    assert_eq!(admin_links.len(), 3);
}

#[tokio::test]
async fn test_click_increment_consistency() {
    // Test that concurrent click increments are consistent
    let storage = create_test_storage().await;
    
    storage.create_with_code("popular", "https://example.com", Some("user1")).await.unwrap();
    
    let mut handles = vec![];
    
    // Increment clicks concurrently
    for _ in 0..100 {
        let storage_clone = Arc::clone(&storage);
        let handle = tokio::spawn(async move {
            storage_clone.increment_click("popular").await
        });
        handles.push(handle);
    }
    
    // Wait for all increments
    for handle in handles {
        handle.await.unwrap().unwrap();
    }
    
    // Verify total click count
    let url = storage.get_authoritative("popular").await.unwrap().unwrap();
    assert_eq!(url.clicks, 100, "All 100 clicks should be counted");
}

#[tokio::test]
async fn test_patch_operations_isolation() {
    // Test that patch operations don't affect unrelated URLs
    let storage = create_test_storage().await;
    
    // Create URLs with various created_by values
    storage.create_with_code("normal1", "https://example.com", Some("user1")).await.unwrap();
    storage.create_with_code("normal2", "https://example.com", Some("user2")).await.unwrap();
    storage.create_with_code("malformed1", "https://example.com", Some("00000000-0000-0000-0000-000000000000")).await.unwrap();
    storage.create_with_code("malformed2", "https://example.com", None).await.unwrap();
    
    // Patch specific URL
    storage.patch_created_by("malformed1", "fixed_user").await.unwrap();
    
    // Verify only targeted URL was changed
    assert_eq!(
        storage.get_authoritative("normal1").await.unwrap().unwrap().created_by,
        Some("user1".to_string())
    );
    assert_eq!(
        storage.get_authoritative("normal2").await.unwrap().unwrap().created_by,
        Some("user2".to_string())
    );
    assert_eq!(
        storage.get_authoritative("malformed1").await.unwrap().unwrap().created_by,
        Some("fixed_user".to_string())
    );
    assert_eq!(
        storage.get_authoritative("malformed2").await.unwrap().unwrap().created_by,
        None
    );
    
    // Patch all malformed
    let count = storage.patch_all_malformed_created_by("system").await.unwrap();
    assert_eq!(count, 1, "Only malformed2 should be patched");
    
    // Verify
    assert_eq!(
        storage.get_authoritative("normal1").await.unwrap().unwrap().created_by,
        Some("user1".to_string())
    );
    assert_eq!(
        storage.get_authoritative("normal2").await.unwrap().unwrap().created_by,
        Some("user2".to_string())
    );
    assert_eq!(
        storage.get_authoritative("malformed1").await.unwrap().unwrap().created_by,
        Some("fixed_user".to_string()),
        "Should not be re-patched"
    );
    assert_eq!(
        storage.get_authoritative("malformed2").await.unwrap().unwrap().created_by,
        Some("system".to_string())
    );
}

#[tokio::test]
async fn test_list_user_links_pagination() {
    // Test pagination for user-specific link listing
    let storage = create_test_storage().await;
    
    // Create 15 links for user1
    for i in 0..15 {
        storage
            .create_with_code(&format!("user1_link{}", i), "https://example.com", Some("user1"))
            .await
            .unwrap();
    }
    
    // Get first page
    let page1 = storage.list_user_links("user1", 5, 0).await.unwrap();
    assert_eq!(page1.len(), 5);
    
    // Get second page
    let page2 = storage.list_user_links("user1", 5, 5).await.unwrap();
    assert_eq!(page2.len(), 5);
    
    // Get third page
    let page3 = storage.list_user_links("user1", 5, 10).await.unwrap();
    assert_eq!(page3.len(), 5);
    
    // Get fourth page (should be empty)
    let page4 = storage.list_user_links("user1", 5, 15).await.unwrap();
    assert_eq!(page4.len(), 0);
    
    // Verify pages don't overlap
    let codes1: Vec<_> = page1.iter().map(|u| u.short_code.as_str()).collect();
    let codes2: Vec<_> = page2.iter().map(|u| u.short_code.as_str()).collect();
    let codes3: Vec<_> = page3.iter().map(|u| u.short_code.as_str()).collect();
    
    for code in &codes2 {
        assert!(!codes1.contains(code));
    }
    for code in &codes3 {
        assert!(!codes1.contains(code));
        assert!(!codes2.contains(code));
    }
}
