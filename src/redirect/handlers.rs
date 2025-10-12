use axum::{
    extract::{Path, State},
    http::{header::HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

use crate::storage::Storage;

pub struct RedirectState {
    pub storage: Arc<dyn Storage>,
}

/// Redirect to original URL
pub async fn redirect_url(
    State(state): State<Arc<RedirectState>>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    let start = Instant::now();
    
    // Measure lookup time
    let lookup_start = Instant::now();
    let result = state.storage.get(&code).await;
    let lookup_time = lookup_start.elapsed();
    
    // Heuristic: if lookup is very fast (<1ms), likely a cache hit
    // This is a reasonable assumption since cache lookups are in-memory
    // while DB lookups require network/disk I/O
    let cache_hit = lookup_time.as_micros() < 1000;
    let cache_time_ms = if cache_hit { lookup_time.as_millis() } else { 0 };
    let db_time_ms = if cache_hit { 0 } else { lookup_time.as_millis() };
    
    match result {
        Ok(Some(url)) => {
            // Check if URL is active
            if !url.is_active {
                return (StatusCode::GONE, "This link has been deactivated").into_response();
            }

            // Increment clicks asynchronously (fire and forget)
            let storage = Arc::clone(&state.storage);
            let code_clone = code.clone();
            tokio::spawn(async move {
                let _ = storage.increment_click(&code_clone).await;
            });

            let total_time = start.elapsed();
            
            // Create headers with tracing info
            let mut headers = HeaderMap::new();
            headers.insert(
                "x-lynx-cache-hit",
                if cache_hit { "true" } else { "false" }
                    .parse()
                    .unwrap(),
            );
            headers.insert(
                "x-lynx-timing-total-ms",
                total_time.as_millis().to_string().parse().unwrap(),
            );
            headers.insert(
                "x-lynx-timing-lookup-ms",
                lookup_time.as_millis().to_string().parse().unwrap(),
            );
            headers.insert(
                "x-lynx-timing-cache-ms",
                cache_time_ms.to_string().parse().unwrap(),
            );
            headers.insert(
                "x-lynx-timing-db-ms",
                db_time_ms.to_string().parse().unwrap(),
            );

            // Redirect with headers
            (headers, Redirect::permanent(&url.original_url)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "URL not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response(),
    }
}

/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    #[derive(Serialize)]
    struct HealthResponse {
        status: String,
    }
    
    Json(HealthResponse {
        status: "OK".to_string(),
    })
}
