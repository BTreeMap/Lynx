//! Integration tests for analytics API endpoints
//!
//! These tests verify that the analytics API endpoints work correctly end-to-end,
//! including the near real-time analytics feature that combines database and
//! in-memory data.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use lynx::analytics::AnalyticsAggregator;
use lynx::auth::AuthService;
use lynx::config::{AuthConfig, AuthMode, Config};
use lynx::storage::{SqliteStorage, Storage};
use serde_json::Value;
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
        short_code_max_length: 50,
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
        redirect_status: RedirectMode::default(),
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
async fn test_analytics_api_endpoint_basic() {
    let storage = create_test_storage().await;
    let auth_service = create_test_auth_service().await;
    let config = create_test_config();

    // Create a short URL
    storage
        .create_with_code("test123", "https://example.com", Some("user1"))
        .await
        .unwrap();

    // Insert some analytics data
    let time_bucket = 1698768000;
    let records = vec![
        (
            "test123".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("CA".to_string()),
            Some("San Francisco".to_string()),
            Some(15169),
            4,
            5,
        ),
        (
            "test123".to_string(),
            time_bucket,
            Some("GB".to_string()),
            Some("England".to_string()),
            Some("London".to_string()),
            Some(16509),
            4,
            3,
        ),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();

    // Create API router without analytics aggregator
    let app = lynx::api::create_api_router(Arc::clone(&storage), auth_service, config, None);

    // Test GET /api/analytics/test123
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/analytics/test123")
                .header(header::AUTHORIZATION, "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 2);
    assert!(json["entries"].is_array());
}

#[tokio::test]
async fn test_analytics_aggregate_api_endpoint() {
    let storage = create_test_storage().await;
    let auth_service = create_test_auth_service().await;
    let config = create_test_config();

    // Create a short URL
    storage
        .create_with_code("multi", "https://example.com", Some("user1"))
        .await
        .unwrap();

    // Insert analytics from multiple countries
    let time_bucket = 1698768000;
    let records = vec![
        (
            "multi".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("CA".to_string()),
            Some("SF".to_string()),
            Some(15169),
            4,
            10,
        ),
        (
            "multi".to_string(),
            time_bucket,
            Some("GB".to_string()),
            Some("England".to_string()),
            Some("London".to_string()),
            Some(16509),
            4,
            5,
        ),
        (
            "multi".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("NY".to_string()),
            Some("NYC".to_string()),
            Some(15169),
            4,
            3,
        ),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();

    // Create API router without analytics aggregator
    let app = lynx::api::create_api_router(Arc::clone(&storage), auth_service, config, None);

    // Test GET /api/analytics/multi/aggregate?group_by=country
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/analytics/multi/aggregate?group_by=country")
                .header(header::AUTHORIZATION, "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 2); // US and GB

    let aggregates = json["aggregates"].as_array().unwrap();
    assert_eq!(aggregates.len(), 2);

    // Verify US has 13 visits (10 + 3)
    let us_agg = aggregates.iter().find(|a| a["dimension"] == "US").unwrap();
    assert_eq!(us_agg["visit_count"], 13);

    // Verify GB has 5 visits
    let gb_agg = aggregates.iter().find(|a| a["dimension"] == "GB").unwrap();
    assert_eq!(gb_agg["visit_count"], 5);
}

#[tokio::test]
async fn test_analytics_aggregate_with_aggregator_realtime() {
    let storage = create_test_storage().await;
    let auth_service = create_test_auth_service().await;
    let config = create_test_config();

    // Create a short URL
    storage
        .create_with_code("realtime", "https://example.com", Some("user1"))
        .await
        .unwrap();

    // Insert some data in database
    let time_bucket = 1698768000;
    let records = vec![(
        "realtime".to_string(),
        time_bucket,
        Some("US".to_string()),
        Some("CA".to_string()),
        Some("SF".to_string()),
        Some(15169),
        4,
        10,
    )];
    storage.upsert_analytics_batch(records).await.unwrap();

    // Create aggregator and add some in-memory data
    let aggregator = Arc::new(AnalyticsAggregator::new());

    // Simulate some pending analytics in memory
    use lynx::analytics::{AnalyticsRecord, GeoLocation};
    let record = AnalyticsRecord {
        short_code: "realtime".to_string(),
        timestamp: time_bucket,
        geo_location: GeoLocation {
            country_code: Some("GB".to_string()),
            country_name: Some("United Kingdom".to_string()),
            region: Some("England".to_string()),
            city: Some("London".to_string()),
            asn: Some(16509),
            asn_org: None,
            ip_version: 4,
        },
        client_ip: None,
    };

    // Record 5 visits for GB (in memory)
    for _ in 0..5 {
        aggregator.record(record.clone());
    }

    // Create API router WITH analytics aggregator
    let app = lynx::api::create_api_router(
        Arc::clone(&storage),
        auth_service,
        config,
        Some(Arc::clone(&aggregator)),
    );

    // Test GET /api/analytics/realtime/aggregate?group_by=country
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/analytics/realtime/aggregate?group_by=country")
                .header(header::AUTHORIZATION, "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 2); // US (from DB) and GB (from memory)

    let aggregates = json["aggregates"].as_array().unwrap();
    assert_eq!(aggregates.len(), 2);

    // Verify US has 10 visits (from database)
    let us_agg = aggregates.iter().find(|a| a["dimension"] == "US").unwrap();
    assert_eq!(us_agg["visit_count"], 10);

    // Verify GB has 5 visits (from in-memory aggregator)
    let gb_agg = aggregates.iter().find(|a| a["dimension"] == "GB").unwrap();
    assert_eq!(gb_agg["visit_count"], 5);
}

#[tokio::test]
async fn test_analytics_aggregate_with_unknown_pending_events() {
    let storage = create_test_storage().await;
    let auth_service = create_test_auth_service().await;
    let config = create_test_config();

    // Create a short URL
    storage
        .create_with_code("pending", "https://example.com", Some("user1"))
        .await
        .unwrap();

    // Create aggregator
    let aggregator = Arc::new(AnalyticsAggregator::new());

    // Record some events that are still in Layer 2 (pending GeoIP lookup)
    use lynx::analytics::AnalyticsEvent;
    use std::net::IpAddr;

    for _ in 0..7 {
        let event = AnalyticsEvent {
            short_code: "pending".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            client_ip: "8.8.8.8".parse::<IpAddr>().unwrap(),
        };
        aggregator.record_event(event);
    }

    // Poll until events are processed into shared buffer (up to 1 second)
    // The actor flushes every 100ms, so this should complete quickly
    let mut attempts = 0;
    let max_attempts = 10; // 10 attempts * 100ms = 1 second max
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check if events have been flushed to shared buffer
        let in_memory = aggregator.get_in_memory_aggregate("pending", "country");
        if !in_memory.is_empty() {
            break;
        }

        attempts += 1;
        if attempts >= max_attempts {
            panic!("Events not flushed to shared buffer after 1 second");
        }
    }

    // Create API router WITH analytics aggregator
    let app = lynx::api::create_api_router(
        Arc::clone(&storage),
        auth_service,
        config,
        Some(Arc::clone(&aggregator)),
    );

    // Test GET /api/analytics/pending/aggregate?group_by=country
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/analytics/pending/aggregate?group_by=country")
                .header(header::AUTHORIZATION, "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Should have 1 entry for "Unknown" (pending GeoIP data)
    assert_eq!(json["total"], 1);

    let aggregates = json["aggregates"].as_array().unwrap();
    assert_eq!(aggregates.len(), 1);

    // Verify Unknown has 7 visits (pending events)
    assert_eq!(aggregates[0]["dimension"], "Unknown");
    assert_eq!(aggregates[0]["visit_count"], 7);
}

#[tokio::test]
async fn test_analytics_aggregate_with_time_range() {
    let storage = create_test_storage().await;
    let auth_service = create_test_auth_service().await;
    let config = create_test_config();

    // Create a short URL
    storage
        .create_with_code("timed", "https://example.com", Some("user1"))
        .await
        .unwrap();

    // Insert records with different time buckets
    let records = vec![
        (
            "timed".to_string(),
            1000,
            Some("US".to_string()),
            Some("CA".to_string()),
            None,
            None,
            4,
            5,
        ),
        (
            "timed".to_string(),
            2000,
            Some("GB".to_string()),
            Some("England".to_string()),
            None,
            None,
            4,
            3,
        ),
        (
            "timed".to_string(),
            3000,
            Some("US".to_string()),
            Some("NY".to_string()),
            None,
            None,
            4,
            7,
        ),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();

    // Create API router
    let app = lynx::api::create_api_router(Arc::clone(&storage), auth_service, config, None);

    // Test with time range that includes only middle record
    let response = app
        .oneshot(
            Request::builder()
                .uri(
                    "/api/analytics/timed/aggregate?group_by=country&start_time=1500&end_time=2500",
                )
                .header(header::AUTHORIZATION, "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 1);

    let aggregates = json["aggregates"].as_array().unwrap();
    assert_eq!(aggregates.len(), 1);
    assert_eq!(aggregates[0]["dimension"], "GB");
    assert_eq!(aggregates[0]["visit_count"], 3);
}

#[tokio::test]
async fn test_analytics_aggregate_group_by_region() {
    let storage = create_test_storage().await;
    let auth_service = create_test_auth_service().await;
    let config = create_test_config();

    // Create a short URL
    storage
        .create_with_code("regions", "https://example.com", Some("user1"))
        .await
        .unwrap();

    // Insert analytics with different regions
    let time_bucket = 1698768000;
    let records = vec![
        (
            "regions".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("CA".to_string()),
            Some("LA".to_string()),
            None,
            4,
            7,
        ),
        (
            "regions".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("NY".to_string()),
            Some("NYC".to_string()),
            None,
            4,
            4,
        ),
        (
            "regions".to_string(),
            time_bucket,
            Some("US".to_string()),
            Some("CA".to_string()),
            Some("SF".to_string()),
            None,
            4,
            2,
        ),
    ];
    storage.upsert_analytics_batch(records).await.unwrap();

    // Create API router
    let app = lynx::api::create_api_router(Arc::clone(&storage), auth_service, config, None);

    // Test group by region
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/analytics/regions/aggregate?group_by=region")
                .header(header::AUTHORIZATION, "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["total"], 2); // CA, US and NY, US

    let aggregates = json["aggregates"].as_array().unwrap();

    // CA should have 9 visits (7 + 2) and be formatted as "CA, US"
    let ca_agg = aggregates
        .iter()
        .find(|a| a["dimension"] == "CA, US")
        .unwrap();
    assert_eq!(ca_agg["visit_count"], 9);

    // NY should have 4 visits and be formatted as "NY, US"
    let ny_agg = aggregates
        .iter()
        .find(|a| a["dimension"] == "NY, US")
        .unwrap();
    assert_eq!(ny_agg["visit_count"], 4);
}
