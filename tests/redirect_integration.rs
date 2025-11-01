//! Redirect integration tests
//!
//! These tests verify that the redirect functionality works correctly,
//! including concurrent redirects and proper handling of active/inactive URLs.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use lynx::redirect;
use lynx::storage::{SqliteStorage, Storage};
use std::sync::Arc;
use tower::ServiceExt;

/// Helper to create test storage
async fn create_test_storage() -> Arc<dyn Storage> {
    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    Arc::new(storage)
}

#[tokio::test]
async fn test_redirect_active_url() {
    // Test basic redirect functionality for an active URL
    let storage = create_test_storage().await;
    
    // Create a test URL
    storage
        .create_with_code("redirect_test", "https://example.com/destination", None)
        .await
        .unwrap();

    let app = redirect::routes::create_redirect_router(storage.clone(), None, None, None, false);

    let request = Request::builder()
        .uri("/redirect_test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should redirect (307 or 302)
    assert!(
        response.status() == StatusCode::TEMPORARY_REDIRECT 
        || response.status() == StatusCode::FOUND,
        "Should return redirect status, got: {}",
        response.status()
    );

    // Verify click was incremented
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let url = storage.get_authoritative("redirect_test").await.unwrap().unwrap();
    assert!(url.clicks >= 1, "Click count should be at least 1");
}

#[tokio::test]
async fn test_redirect_inactive_url() {
    // Test that inactive URLs return 404
    let storage = create_test_storage().await;
    
    // Create and deactivate a URL
    storage
        .create_with_code("inactive_test", "https://example.com", None)
        .await
        .unwrap();
    
    storage.deactivate("inactive_test").await.unwrap();

    let app = redirect::routes::create_redirect_router(storage.clone(), None, None, None, false);

    let request = Request::builder()
        .uri("/inactive_test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 for inactive URL
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Inactive URL should return 404"
    );
}

#[tokio::test]
async fn test_redirect_nonexistent_url() {
    // Test that nonexistent short codes return 404
    let storage = create_test_storage().await;
    let app = redirect::routes::create_redirect_router(storage.clone(), None, None, None, false);

    let request = Request::builder()
        .uri("/nonexistent")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Nonexistent URL should return 404"
    );
}

#[tokio::test]
async fn test_concurrent_redirects() {
    // Test that concurrent redirects to the same URL work correctly
    let storage = create_test_storage().await;
    
    // Create a test URL
    storage
        .create_with_code("popular", "https://example.com", None)
        .await
        .unwrap();

    let app = redirect::routes::create_redirect_router(storage.clone(), None, None, None, false);

    // Spawn many concurrent redirect requests
    let mut handles = vec![];
    
    for _ in 0..50 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .uri("/popular")
                .body(Body::empty())
                .unwrap();

            app_clone.oneshot(request).await
        });
        handles.push(handle);
    }

    // All should succeed with redirect status
    let mut success_count = 0;
    
    for handle in handles {
        match handle.await {
            Ok(Ok(response)) => {
                if response.status() == StatusCode::TEMPORARY_REDIRECT
                    || response.status() == StatusCode::FOUND
                {
                    success_count += 1;
                }
            }
            _ => {}
        }
    }

    assert_eq!(success_count, 50, "All 50 redirects should succeed");

    // Verify clicks were counted (may take a moment due to async batching)
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    let url = storage.get_authoritative("popular").await.unwrap().unwrap();
    assert!(
        url.clicks >= 50,
        "Should have at least 50 clicks, got {}",
        url.clicks
    );
}

#[tokio::test]
async fn test_redirect_during_deactivation() {
    // Test race condition between redirects and deactivation
    let storage = create_test_storage().await;
    
    // Create a test URL
    storage
        .create_with_code("race_redirect", "https://example.com", None)
        .await
        .unwrap();

    let app = redirect::routes::create_redirect_router(storage.clone(), None, None, None, false);

    // Spawn redirect tasks
    let mut redirect_handles = vec![];
    for _ in 0..20 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .uri("/race_redirect")
                .body(Body::empty())
                .unwrap();

            app_clone.oneshot(request).await
        });
        redirect_handles.push(handle);
    }

    // Deactivate in the middle
    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    storage.deactivate("race_redirect").await.unwrap();

    // Some redirects may succeed (before deactivation), some may fail (after)
    let mut redirect_success = 0;
    let mut not_found = 0;
    
    for handle in redirect_handles {
        if let Ok(Ok(response)) = handle.await {
            match response.status() {
                StatusCode::TEMPORARY_REDIRECT | StatusCode::FOUND => redirect_success += 1,
                StatusCode::NOT_FOUND => not_found += 1,
                _ => {}
            }
        }
    }

    // Should have some of each (race condition)
    assert!(
        redirect_success + not_found == 20,
        "All requests should complete with either redirect or 404"
    );

    // Final state should be inactive
    let url = storage.get_authoritative("race_redirect").await.unwrap().unwrap();
    assert!(!url.is_active, "URL should be inactive");
}

#[tokio::test]
async fn test_redirect_multiple_different_urls() {
    // Test concurrent redirects to different URLs
    let storage = create_test_storage().await;
    
    // Create multiple test URLs
    for i in 0..10 {
        storage
            .create_with_code(
                &format!("url_{}", i),
                &format!("https://example.com/{}", i),
                None,
            )
            .await
            .unwrap();
    }

    let app = redirect::routes::create_redirect_router(storage.clone(), None, None, None, false);

    // Spawn concurrent redirects to different URLs
    let mut handles = vec![];
    
    for i in 0..10 {
        for _ in 0..5 {
            let app_clone = app.clone();
            let url_path = format!("/url_{}", i);
            let handle = tokio::spawn(async move {
                let request = Request::builder()
                    .uri(&url_path)
                    .body(Body::empty())
                    .unwrap();

                app_clone.oneshot(request).await
            });
            handles.push(handle);
        }
    }

    // All 50 redirects should succeed
    let mut success_count = 0;
    
    for handle in handles {
        if let Ok(Ok(response)) = handle.await {
            if response.status() == StatusCode::TEMPORARY_REDIRECT
                || response.status() == StatusCode::FOUND
            {
                success_count += 1;
            }
        }
    }

    assert_eq!(success_count, 50, "All 50 redirects should succeed");
}
