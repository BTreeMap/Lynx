//! Concurrent API integration tests
//!
//! These tests verify that the API correctly handles concurrent operations,
//! particularly for short code creation which is a critical operation.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use lynx::api;
use lynx::auth::AuthService;
use lynx::config::{AuthConfig, AuthMode, Config};
use lynx::storage::{SqliteStorage, Storage};
use std::sync::Arc;
use tower::ServiceExt;

/// Helper to create test storage
async fn create_test_storage() -> Arc<dyn Storage> {
    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    Arc::new(storage)
}

/// Helper to create test config
fn create_test_config() -> Arc<Config> {
    use lynx::config::*;

    Arc::new(Config {
        database: DatabaseConfig {
            backend: DatabaseBackend::Sqlite,
            url: "sqlite::memory:".to_string(),
            max_connections: 5,
        },
        api_server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        redirect_server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
        },
        redirect_base_url: "http://localhost:3000".to_string(),
        auth: AuthConfig {
            mode: AuthMode::None,
            oauth: None,
            cloudflare: None,
        },
        frontend: FrontendConfig { static_dir: None },
        cache: CacheConfig {
            max_entries: 10000,
            flush_interval_secs: 5,
            actor_buffer_size: 100000,
            actor_flush_interval_ms: 100,
        },
        pagination: PaginationConfig {
            cursor_hmac_secret: None,
        },
        analytics: AnalyticsConfig {
            enabled: false,
            geoip_city_db_path: None,
            geoip_asn_db_path: None,
            ip_anonymization: false,
            trusted_proxy_mode: TrustedProxyMode::None,
            trusted_proxies: vec![],
            num_trusted_proxies: None,
            flush_interval_secs: 30,
        },
    })
}

/// Helper to create test auth service
async fn create_test_auth_service() -> Arc<AuthService> {
    let config = AuthConfig {
        mode: AuthMode::None,
        oauth: None,
        cloudflare: None,
    };
    Arc::new(AuthService::new(config).await.unwrap())
}

#[tokio::test]
async fn test_concurrent_short_code_creation() {
    // Test that concurrent short code creation handles conflicts correctly
    let storage = create_test_storage().await;
    let config = create_test_config();
    let auth_service = create_test_auth_service().await;
    
    let app = api::routes::create_api_router(
        storage.clone(),
        auth_service,
        config.clone(),
        None,
    );

    // Spawn multiple concurrent requests to create the same short code
    let mut handles = vec![];
    
    for i in 0..10 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .method("POST")
                .uri("/api/shorten")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"url": "https://example.com", "custom_code": "concurrent_test"}}"#
                )))
                .unwrap();

            app_clone.oneshot(request).await.unwrap()
        });
        handles.push((i, handle));
    }

    // Collect results
    let mut success_count = 0;
    let mut conflict_count = 0;
    
    for (i, handle) in handles {
        match handle.await {
            Ok(response) => {
                let status = response.status();
                if status == StatusCode::OK || status == StatusCode::CREATED {
                    success_count += 1;
                } else if status == StatusCode::CONFLICT {
                    conflict_count += 1;
                } else {
                    println!("Request {} got unexpected status: {}", i, status);
                }
            }
            Err(e) => {
                panic!("Request {} failed: {:?}", i, e);
            }
        }
    }

    // Exactly one should succeed, others should get conflict
    assert_eq!(success_count, 1, "Exactly one creation should succeed");
    assert_eq!(conflict_count, 9, "All others should get conflict (409)");
}

#[tokio::test]
async fn test_concurrent_different_short_codes() {
    // Test that concurrent creation of different short codes all succeed
    let storage = create_test_storage().await;
    let config = create_test_config();
    let auth_service = create_test_auth_service().await;
    
    let app = api::routes::create_api_router(
        storage.clone(),
        auth_service,
        config.clone(),
        None,
    );

    // Spawn multiple concurrent requests with different short codes
    let mut handles = vec![];
    
    for i in 0..10 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .method("POST")
                .uri("/api/shorten")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"url": "https://example.com", "custom_code": "test_{:03}"}}"#,
                    i
                )))
                .unwrap();

            app_clone.oneshot(request).await.unwrap()
        });
        handles.push(handle);
    }

    // All should succeed
    let mut success_count = 0;
    
    for handle in handles {
        match handle.await {
            Ok(response) => {
                let status = response.status();
                if status == StatusCode::OK || status == StatusCode::CREATED {
                    success_count += 1;
                }
            }
            Err(e) => {
                panic!("Request failed: {:?}", e);
            }
        }
    }

    assert_eq!(success_count, 10, "All 10 creations should succeed");
}

#[tokio::test]
async fn test_concurrent_url_lookups() {
    // Test that concurrent lookups of the same URL work correctly
    let storage = create_test_storage().await;
    
    // Create a test URL first
    storage
        .create_with_code("lookup_test", "https://example.com", None)
        .await
        .unwrap();

    // Spawn many concurrent lookup requests
    let mut handles = vec![];
    
    for _ in 0..50 {
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            storage_clone.get("lookup_test").await
        });
        handles.push(handle);
    }

    // All should succeed and return the same URL
    let mut success_count = 0;
    
    for handle in handles {
        match handle.await {
            Ok(Ok(result)) => {
                if result.url.is_some() {
                    success_count += 1;
                    assert_eq!(result.url.unwrap().short_code, "lookup_test");
                }
            }
            _ => {}
        }
    }

    assert_eq!(success_count, 50, "All 50 lookups should succeed");
}

#[tokio::test]
async fn test_concurrent_click_increments() {
    // Test that concurrent click increments are correctly counted
    let storage = create_test_storage().await;
    
    // Create a test URL
    storage
        .create_with_code("click_test", "https://example.com", None)
        .await
        .unwrap();

    // Spawn many concurrent increment requests
    let mut handles = vec![];
    
    for _ in 0..100 {
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            storage_clone.increment_click("click_test").await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // Verify the count is correct
    let url = storage.get_authoritative("click_test").await.unwrap().unwrap();
    assert_eq!(url.clicks, 100, "Should have exactly 100 clicks");
}

#[tokio::test]
async fn test_concurrent_deactivate_and_lookup() {
    // Test race condition between deactivation and lookups
    let storage = create_test_storage().await;
    
    // Create a test URL
    storage
        .create_with_code("race_test", "https://example.com", None)
        .await
        .unwrap();

    let storage_clone = storage.clone();
    
    // Spawn a deactivation task
    let deactivate_handle = tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        storage_clone.deactivate("race_test").await
    });

    // Spawn multiple lookup tasks
    let mut lookup_handles = vec![];
    for _ in 0..20 {
        let storage_clone = storage.clone();
        let handle = tokio::spawn(async move {
            storage_clone.get_authoritative("race_test").await
        });
        lookup_handles.push(handle);
    }

    // Wait for deactivation
    deactivate_handle.await.unwrap().unwrap();

    // Check lookups - some may see active, some inactive, but all should succeed
    let mut found_inactive = false;
    
    for handle in lookup_handles {
        if let Ok(Ok(Some(url))) = handle.await {
            if !url.is_active {
                found_inactive = true;
            }
        }
    }

    // Should have found at least the inactive state after deactivation completed
    assert!(found_inactive, "Should have found inactive state");
    
    // Final state should be inactive
    let final_url = storage.get_authoritative("race_test").await.unwrap().unwrap();
    assert!(!final_url.is_active, "Final state should be inactive");
}
