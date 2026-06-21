//! API integration tests for version-controlled URL updates.
//!
//! These exercise the HTTP surface for the destination-history feature:
//! `PATCH /api/urls/{code}`, `GET /api/urls/{code}/history`, and
//! `POST /api/urls/{code}/history/{history_id}/restore`.
//!
//! Tests run with `AUTH_MODE=none`, so the caller is always treated as an
//! admin. Fine-grained owner/admin authorization is covered by unit tests in
//! `src/api/handlers.rs`.

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use lynx::api;
use lynx::auth::AuthService;
use lynx::config::*;
use lynx::storage::{SqliteStorage, Storage};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn create_test_storage() -> Arc<dyn Storage> {
    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    Arc::new(storage)
}

fn create_test_config() -> Arc<Config> {
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

async fn create_test_auth_service() -> Arc<AuthService> {
    let config = AuthConfig {
        mode: AuthMode::None,
        oauth: None,
        cloudflare: None,
    };
    Arc::new(AuthService::new(config).await.unwrap())
}

async fn build_app() -> Router {
    let storage = create_test_storage().await;
    let config = create_test_config();
    let auth_service = create_test_auth_service().await;
    api::routes::create_api_router(storage, auth_service, config, None)
}

fn encode_short_code(code: &str) -> String {
    URL_SAFE_NO_PAD.encode(code.as_bytes())
}

/// Send a JSON request and return the status plus the parsed JSON body.
async fn send(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => {
            // PATCH/POST handlers expect a JSON body; default to an empty object.
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            builder.body(Body::from("{}")).unwrap()
        }
    };

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

async fn create_url(app: &Router, code: &str, url: &str) {
    let (status, _) = send(
        app,
        "POST",
        "/api/urls",
        Some(json!({ "url": url, "custom_code": code })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "URL creation should succeed");
}

#[tokio::test]
async fn test_update_records_history_and_changes_destination() {
    let app = build_app().await;
    create_url(&app, "hist", "https://v1.example.com").await;

    let encoded = encode_short_code("hist");

    // No history before any change.
    let (status, body) = send(&app, "GET", &format!("/api/urls/{encoded}/history"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);

    // Update the destination.
    let (status, body) = send(
        &app,
        "PATCH",
        &format!("/api/urls/{encoded}"),
        Some(json!({ "url": "https://v2.example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["original_url"], "https://v2.example.com");

    // The read path now serves the new destination.
    let (status, body) = send(&app, "GET", &format!("/api/urls/{encoded}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["original_url"], "https://v2.example.com");

    // History captured the previous destination.
    let (status, body) = send(&app, "GET", &format!("/api/urls/{encoded}/history"), None).await;
    assert_eq!(status, StatusCode::OK);
    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["historic_url"], "https://v1.example.com");
}

#[tokio::test]
async fn test_history_is_ordered_newest_first() {
    let app = build_app().await;
    create_url(&app, "multi", "https://v1.example.com").await;
    let encoded = encode_short_code("multi");

    for next in ["https://v2.example.com", "https://v3.example.com"] {
        let (status, _) = send(
            &app,
            "PATCH",
            &format!("/api/urls/{encoded}"),
            Some(json!({ "url": next })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (status, body) = send(&app, "GET", &format!("/api/urls/{encoded}/history"), None).await;
    assert_eq!(status, StatusCode::OK);
    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["historic_url"], "https://v2.example.com");
    assert_eq!(entries[1]["historic_url"], "https://v1.example.com");
}

#[tokio::test]
async fn test_restore_reverts_destination() {
    let app = build_app().await;
    create_url(&app, "restore", "https://v1.example.com").await;
    let encoded = encode_short_code("restore");

    for next in ["https://v2.example.com", "https://v3.example.com"] {
        send(
            &app,
            "PATCH",
            &format!("/api/urls/{encoded}"),
            Some(json!({ "url": next })),
        )
        .await;
    }

    // Oldest history entry holds the original destination.
    let (_, body) = send(&app, "GET", &format!("/api/urls/{encoded}/history"), None).await;
    let entries = body.as_array().unwrap();
    let original_id = entries.last().unwrap()["id"].as_i64().unwrap();
    assert_eq!(
        entries.last().unwrap()["historic_url"],
        "https://v1.example.com"
    );

    // Restore to the original destination.
    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/urls/{encoded}/history/{original_id}/restore"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["original_url"], "https://v1.example.com");

    // The currently-active destination (v3) was preserved in history.
    let (_, body) = send(&app, "GET", &format!("/api/urls/{encoded}/history"), None).await;
    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["historic_url"], "https://v3.example.com");
}

#[tokio::test]
async fn test_update_missing_code_returns_404() {
    let app = build_app().await;
    let encoded = encode_short_code("nope");
    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/urls/{encoded}"),
        Some(json!({ "url": "https://example.com" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_empty_url_returns_400() {
    let app = build_app().await;
    create_url(&app, "empty", "https://v1.example.com").await;
    let encoded = encode_short_code("empty");
    let (status, _) = send(
        &app,
        "PATCH",
        &format!("/api/urls/{encoded}"),
        Some(json!({ "url": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_restore_unknown_history_returns_404() {
    let app = build_app().await;
    create_url(&app, "badrestore", "https://v1.example.com").await;
    let encoded = encode_short_code("badrestore");
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/urls/{encoded}/history/999999/restore"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
