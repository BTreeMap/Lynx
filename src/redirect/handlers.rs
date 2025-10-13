use axum::{
    extract::{Path, State},
    http::{header::HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Extension, Json,
};
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

use super::middleware::RequestStart;
use crate::storage::Storage;

pub struct RedirectState {
    pub storage: Arc<dyn Storage>,
}

/// Redirect to original URL
pub async fn redirect_url(
    State(state): State<Arc<RedirectState>>,
    Path(code): Path<String>,
    Extension(RequestStart(request_start)): Extension<RequestStart>,
) -> impl IntoResponse {
    let handler_start = Instant::now();

    // Get URL with metadata
    let lookup_result = state.storage.get(&code).await;

    match lookup_result {
        Ok(result) => {
            let cache_hit = result.metadata.cache_hit;
            let cache_time_ms = result
                .metadata
                .cache_duration
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let db_time_ms = result
                .metadata
                .db_duration
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            match result.url {
                Some(url) => {
                    // Check if URL is active
                    if !url.is_active {
                        return (StatusCode::GONE, "This link has been deactivated")
                            .into_response();
                    }

                    if let Err(err) = state.storage.increment_click(&code).await {
                        tracing::warn!(short_code = %code, error = %err, "failed to buffer click increment");
                    }

                    let handler_time = handler_start.elapsed();
                    let total_time = request_start.elapsed();

                    // Create headers with tracing info
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        "x-lynx-cache-hit",
                        if cache_hit { "true" } else { "false" }.parse().unwrap(),
                    );
                    headers.insert(
                        "x-lynx-timing-total-ms",
                        total_time.as_millis().to_string().parse().unwrap(),
                    );
                    headers.insert(
                        "x-lynx-timing-cache-ms",
                        cache_time_ms.to_string().parse().unwrap(),
                    );
                    headers.insert(
                        "x-lynx-timing-db-ms",
                        db_time_ms.to_string().parse().unwrap(),
                    );
                    headers.insert(
                        "x-lynx-timing-handler-ms",
                        handler_time.as_millis().to_string().parse().unwrap(),
                    );

                    // Redirect with headers
                    (headers, Redirect::permanent(&url.original_url)).into_response()
                }
                None => (StatusCode::NOT_FOUND, "URL not found").into_response(),
            }
        }
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
