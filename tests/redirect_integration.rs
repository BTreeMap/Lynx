//! Redirect integration tests
//!
//! These tests verify that the redirect functionality works correctly,
//! including concurrent redirects and proper handling of active/inactive URLs.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use lynx::redirect::{self, middleware::RequestStart};
use lynx::storage::{SqliteStorage, Storage};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower::{Layer, ServiceExt};

/// Default redirect status code for tests (308 Permanent Redirect)
const DEFAULT_REDIRECT_STATUS: StatusCode = StatusCode::PERMANENT_REDIRECT;

/// Helper to create test storage
async fn create_test_storage() -> Arc<dyn Storage> {
    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    Arc::new(storage)
}

/// Helper layer to inject ConnectInfo for tests
#[derive(Clone)]
struct TestConnectInfoLayer;

impl<S> Layer<S> for TestConnectInfoLayer {
    type Service = TestConnectInfoMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TestConnectInfoMiddleware { inner }
    }
}

#[derive(Clone)]
struct TestConnectInfoMiddleware<S> {
    inner: S,
}

impl<S, B> tower::Service<Request<B>> for TestConnectInfoMiddleware<S>
where
    S: tower::Service<Request<B>> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        // Insert test ConnectInfo extension
        let addr = SocketAddr::from(([127, 0, 0, 1], 12345));
        req.extensions_mut()
            .insert(axum::extract::connect_info::ConnectInfo(addr));

        // Insert RequestStart extension
        req.extensions_mut().insert(RequestStart(Instant::now()));

        self.inner.call(req)
    }
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

    let app = redirect::routes::create_redirect_router(
        storage.clone(),
        None,
        None,
        None,
        false,
        DEFAULT_REDIRECT_STATUS,
    )
    .layer(TestConnectInfoLayer);

    let request = Request::builder()
        .uri("/redirect_test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should redirect (308 permanent redirect)
    assert_eq!(
        response.status(),
        StatusCode::PERMANENT_REDIRECT,
        "Should return permanent redirect status, got: {}",
        response.status()
    );

    // Verify click was incremented
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let url = storage
        .get_authoritative("redirect_test")
        .await
        .unwrap()
        .unwrap();
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

    let app = redirect::routes::create_redirect_router(
        storage.clone(),
        None,
        None,
        None,
        false,
        DEFAULT_REDIRECT_STATUS,
    )
    .layer(TestConnectInfoLayer);

    let request = Request::builder()
        .uri("/inactive_test")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 410 GONE for inactive URL
    assert_eq!(
        response.status(),
        StatusCode::GONE,
        "Inactive URL should return 410 GONE"
    );
}

#[tokio::test]
async fn test_redirect_nonexistent_url() {
    // Test that nonexistent short codes return 404
    let storage = create_test_storage().await;
    let app = redirect::routes::create_redirect_router(
        storage.clone(),
        None,
        None,
        None,
        false,
        DEFAULT_REDIRECT_STATUS,
    )
    .layer(TestConnectInfoLayer);

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

    let app = redirect::routes::create_redirect_router(
        storage.clone(),
        None,
        None,
        None,
        false,
        DEFAULT_REDIRECT_STATUS,
    )
    .layer(TestConnectInfoLayer);

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
                if response.status() == StatusCode::PERMANENT_REDIRECT {
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

    let app = redirect::routes::create_redirect_router(
        storage.clone(),
        None,
        None,
        None,
        false,
        DEFAULT_REDIRECT_STATUS,
    )
    .layer(TestConnectInfoLayer);

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
    let mut gone_count = 0;

    for handle in redirect_handles {
        if let Ok(Ok(response)) = handle.await {
            match response.status() {
                StatusCode::PERMANENT_REDIRECT => redirect_success += 1,
                StatusCode::GONE => gone_count += 1,
                _ => {}
            }
        }
    }

    // Should have some of each (race condition)
    assert!(
        redirect_success + gone_count == 20,
        "All requests should complete with either redirect or 410 GONE"
    );

    // Final state should be inactive
    let url = storage
        .get_authoritative("race_redirect")
        .await
        .unwrap()
        .unwrap();
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

    let app = redirect::routes::create_redirect_router(
        storage.clone(),
        None,
        None,
        None,
        false,
        DEFAULT_REDIRECT_STATUS,
    )
    .layer(TestConnectInfoLayer);

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
            if response.status() == StatusCode::PERMANENT_REDIRECT {
                success_count += 1;
            }
        }
    }

    assert_eq!(success_count, 50, "All 50 redirects should succeed");
}

#[tokio::test]
async fn test_configurable_redirect_status_codes() {
    // Test that different redirect status codes can be configured
    let storage = create_test_storage().await;

    // Create a test URL
    storage
        .create_with_code("status_test", "https://example.com", None)
        .await
        .unwrap();

    // Test with different status codes
    let test_cases = vec![
        (StatusCode::MOVED_PERMANENTLY, "301"),  // 301
        (StatusCode::FOUND, "302"),              // 302
        (StatusCode::SEE_OTHER, "303"),          // 303
        (StatusCode::TEMPORARY_REDIRECT, "307"), // 307
        (StatusCode::PERMANENT_REDIRECT, "308"), // 308
    ];

    for (status_code, description) in test_cases {
        let app = redirect::routes::create_redirect_router(
            storage.clone(),
            None,
            None,
            None,
            false,
            status_code,
        )
        .layer(TestConnectInfoLayer);

        let request = Request::builder()
            .uri("/status_test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(
            response.status(),
            status_code,
            "Should return {} status code",
            description
        );

        // Verify the Location header is present
        let headers = response.headers();
        assert!(
            headers.contains_key("location"),
            "Response should contain Location header"
        );
    }
}
