use axum::{
    extract::{ConnectInfo, Path, State},
    http::{
        header::{HeaderMap, HeaderValue, LOCATION},
        StatusCode,
    },
    response::IntoResponse,
    Extension, Json,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use super::middleware::RequestStart;
use crate::analytics::{AnalyticsAggregator, GeoIpService};
use crate::config::AnalyticsConfig;
use crate::storage::Storage;

pub struct RedirectState {
    pub storage: Arc<dyn Storage>,
    pub analytics_config: Option<AnalyticsConfig>,
    pub geoip_service: Option<Arc<GeoIpService>>,
    pub analytics_aggregator: Option<Arc<AnalyticsAggregator>>,
    pub enable_timing_headers: bool,
    /// Configurable redirect status code (301/302/303/307/308).
    /// Stored as StatusCode for zero-cost access during redirects.
    pub redirect_status: StatusCode,
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
                    if let (Some(config), Some(geoip), Some(aggregator)) = (
                        &state.analytics_config,
                        &state.geoip_service,
                        &state.analytics_aggregator,
                    ) {
                        if config.enabled {
                            record_analytics(&code, &headers, addr.ip(), config, geoip, aggregator);
                        }
                    }

                    // Pre-calculate the Location header value.
                    // This does the same work as Redirect::permanent (HeaderValue::try_from).
                    // We handle the error gracefully instead of panicking.
                    let location_val = match HeaderValue::try_from(&url.original_url) {
                        Ok(val) => val,
                        Err(e) => {
                            tracing::error!(
                                short_code = %code,
                                url = %url.original_url,
                                error = %e,
                                "Failed to create Location header - URL contains invalid characters"
                            );
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "URL contains invalid characters for HTTP header",
                            )
                                .into_response()
                        }
                    };

                    let handler_time = handler_start.elapsed();
                    let total_time = request_start.elapsed();

                    // Create headers with tracing info (optional for maximum performance)
                    if state.enable_timing_headers {
                        let mut response_headers = HeaderMap::new();
                        response_headers.insert(LOCATION, location_val);
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

                        // Use the configured status code and the populated HeaderMap
                        (state.redirect_status, response_headers).into_response()
                    } else {
                        // Fast path: Construct response directly with an array of headers.
                        // This avoids allocating a HeaderMap and is the fastest possible way to return a redirect.
                        (state.redirect_status, [(LOCATION, location_val)]).into_response()
                    }
                }
                None => (StatusCode::NOT_FOUND, "URL not found").into_response(),
            }
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response(),
    }
}

fn record_analytics(
    short_code: &str,
    headers: &HeaderMap,
    socket_ip: std::net::IpAddr,
    config: &AnalyticsConfig,
    _geoip: &GeoIpService,
    aggregator: &AnalyticsAggregator,
) {
    use crate::analytics::ip_extractor::{anonymize_ip, extract_client_ip};
    use crate::analytics::AnalyticsEvent;

    // Extract client IP based on trust configuration
    let mut client_ip = extract_client_ip(headers, socket_ip, config);

    // Anonymize IP if configured
    if config.ip_anonymization {
        client_ip = anonymize_ip(client_ip);
    }

    // Create lightweight event WITHOUT GeoIP lookup (deferred to flush time)
    // This keeps the hot path fast!
    let event = AnalyticsEvent {
        short_code: short_code.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        client_ip,
    };

    // Record event in aggregator (non-blocking, no GeoIP lookup!)
    aggregator.record_event(event);
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
