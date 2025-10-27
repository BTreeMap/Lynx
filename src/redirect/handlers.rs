use axum::{
    extract::{ConnectInfo, Path, State},
    http::{header::HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Extension, Json,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use super::middleware::RequestStart;
use crate::config::AnalyticsConfig;
use crate::storage::Storage;

#[cfg(feature = "analytics")]
use crate::analytics::{AnalyticsAggregator, AnalyticsRecord, GeoIpService};

pub struct RedirectState {
    pub storage: Arc<dyn Storage>,
    #[cfg(feature = "analytics")]
    pub analytics_config: Option<AnalyticsConfig>,
    #[cfg(feature = "analytics")]
    pub geoip_service: Option<Arc<GeoIpService>>,
    #[cfg(feature = "analytics")]
    pub analytics_aggregator: Option<Arc<AnalyticsAggregator>>,
}

/// Redirect to original URL
pub async fn redirect_url(
    State(state): State<Arc<RedirectState>>,
    Path(code): Path<String>,
    Extension(RequestStart(request_start)): Extension<RequestStart>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
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

                    // Record analytics if enabled
                    #[cfg(feature = "analytics")]
                    if let (Some(config), Some(geoip), Some(aggregator)) = (
                        &state.analytics_config,
                        &state.geoip_service,
                        &state.analytics_aggregator,
                    ) {
                        if config.enabled {
                            record_analytics(
                                &code,
                                &headers,
                                addr.ip(),
                                config,
                                geoip,
                                aggregator,
                            );
                        }
                    }

                    let handler_time = handler_start.elapsed();
                    let total_time = request_start.elapsed();

                    // Create headers with tracing info
                    let mut response_headers = HeaderMap::new();
                    response_headers.insert(
                        "x-lynx-cache-hit",
                        if cache_hit { "true" } else { "false" }.parse().unwrap(),
                    );
                    response_headers.insert(
                        "x-lynx-timing-total-ms",
                        total_time.as_millis().to_string().parse().unwrap(),
                    );
                    response_headers.insert(
                        "x-lynx-timing-cache-ms",
                        cache_time_ms.to_string().parse().unwrap(),
                    );
                    response_headers.insert(
                        "x-lynx-timing-db-ms",
                        db_time_ms.to_string().parse().unwrap(),
                    );
                    response_headers.insert(
                        "x-lynx-timing-handler-ms",
                        handler_time.as_millis().to_string().parse().unwrap(),
                    );

                    // Redirect with headers
                    (response_headers, Redirect::permanent(&url.original_url)).into_response()
                }
                None => (StatusCode::NOT_FOUND, "URL not found").into_response(),
            }
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response(),
    }
}

#[cfg(feature = "analytics")]
fn record_analytics(
    short_code: &str,
    headers: &HeaderMap,
    socket_ip: std::net::IpAddr,
    config: &AnalyticsConfig,
    geoip: &GeoIpService,
    aggregator: &AnalyticsAggregator,
) {
    use crate::analytics::ip_extractor::{anonymize_ip, extract_client_ip};

    // Extract client IP based on trust configuration
    let mut client_ip = extract_client_ip(headers, socket_ip, config);

    // Anonymize IP if configured
    if config.ip_anonymization {
        client_ip = anonymize_ip(client_ip);
    }

    // Lookup geolocation
    let geo_location = geoip.lookup(client_ip);

    // Create analytics record
    let record = AnalyticsRecord {
        short_code: short_code.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        geo_location,
        client_ip: if config.ip_anonymization {
            None // Don't store IP if anonymization is enabled
        } else {
            Some(client_ip)
        },
    };

    // Record in aggregator (non-blocking)
    aggregator.record(record);
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
