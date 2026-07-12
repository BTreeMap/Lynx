use axum::{
    extract::{ConnectInfo, Path, State},
    http::{
        header::{HeaderMap, HeaderValue, LOCATION},
        StatusCode,
    },
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use super::middleware::RequestStart;
use crate::analytics::AnalyticsAggregator;
use crate::config::AnalyticsConfig;
use crate::models::ShortenedUrl;
use crate::storage::{CachedStorage, LookupMetadata, Storage};

#[derive(Clone)]
pub struct RedirectAnalytics {
    config: AnalyticsConfig,
    aggregator: Arc<AnalyticsAggregator>,
}

impl RedirectAnalytics {
    pub fn from_enabled(
        config: AnalyticsConfig,
        aggregator: Arc<AnalyticsAggregator>,
    ) -> Option<Self> {
        config.enabled.then_some(Self { config, aggregator })
    }

    fn record(&self, short_code: &str, headers: &HeaderMap, socket_ip: std::net::IpAddr) {
        record_analytics(
            short_code,
            headers,
            socket_ip,
            &self.config,
            &self.aggregator,
        );
    }
}

pub struct RedirectState {
    pub(super) storage: Arc<CachedStorage>,
    pub(super) analytics: Option<RedirectAnalytics>,
    /// Configurable redirect status code (301/302/303/307/308).
    /// Stored as StatusCode for zero-cost access during redirects.
    pub(super) redirect_status: StatusCode,
}

/// Minimal redirect path used when analytics and timing headers are disabled.
pub async fn redirect_url(
    State(state): State<Arc<RedirectState>>,
    Path(code): Path<String>,
) -> Response {
    match prepare_redirect(&state, &code).await {
        Ok(url) => {
            let response = redirect_response(&state, &url);
            buffer_click(&state, code);
            response
        }
        Err(response) => response,
    }
}

/// Redirect path with analytics but without timing instrumentation.
pub async fn redirect_url_with_analytics(
    State(state): State<Arc<RedirectState>>,
    Path(code): Path<String>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    match prepare_redirect(&state, &code).await {
        Ok(url) => {
            state
                .analytics
                .as_ref()
                .expect("analytics handler requires analytics runtime")
                .record(&url.short_code, &headers, addr.ip());
            let response = redirect_response(&state, &url);
            buffer_click(&state, code);
            response
        }
        Err(response) => response,
    }
}

/// Redirect path with timing headers but without analytics extractors.
pub async fn redirect_url_with_timing(
    State(state): State<Arc<RedirectState>>,
    Path(code): Path<String>,
    Extension(RequestStart(request_start)): Extension<RequestStart>,
) -> Response {
    let handler_start = Instant::now();
    match prepare_measured_redirect(&state, &code).await {
        Ok((url, metadata)) => {
            let response =
                timed_redirect_response(&state, &url, metadata, handler_start, request_start);
            buffer_click(&state, code);
            response
        }
        Err(response) => response,
    }
}

/// Fully instrumented redirect path with analytics and timing headers.
pub async fn redirect_url_with_analytics_and_timing(
    State(state): State<Arc<RedirectState>>,
    Path(code): Path<String>,
    Extension(RequestStart(request_start)): Extension<RequestStart>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let handler_start = Instant::now();
    match prepare_measured_redirect(&state, &code).await {
        Ok((url, metadata)) => {
            state
                .analytics
                .as_ref()
                .expect("analytics handler requires analytics runtime")
                .record(&url.short_code, &headers, addr.ip());
            let response =
                timed_redirect_response(&state, &url, metadata, handler_start, request_start);
            buffer_click(&state, code);
            response
        }
        Err(response) => response,
    }
}

async fn prepare_redirect(
    state: &RedirectState,
    code: &str,
) -> Result<Arc<ShortenedUrl>, Response> {
    let url = state
        .storage
        .get(code)
        .await
        .map_err(|_| internal_error())?;
    accept_redirect(url).map_err(IntoResponse::into_response)
}

async fn prepare_measured_redirect(
    state: &RedirectState,
    code: &str,
) -> Result<(Arc<ShortenedUrl>, LookupMetadata), Response> {
    let result = state
        .storage
        .get_with_metadata(code)
        .await
        .map_err(|_| internal_error())?;
    let url = accept_redirect(result.url).map_err(IntoResponse::into_response)?;
    Ok((url, result.metadata))
}

fn accept_redirect(
    url: Option<Arc<ShortenedUrl>>,
) -> Result<Arc<ShortenedUrl>, (StatusCode, &'static str)> {
    let Some(url) = url else {
        return Err((StatusCode::NOT_FOUND, "URL not found"));
    };
    if !url.is_active {
        return Err((StatusCode::GONE, "This link has been deactivated"));
    }

    Ok(url)
}

fn buffer_click(state: &RedirectState, code: String) {
    if let Err(error) = state.storage.buffer_click_owned(code, 1) {
        tracing::warn!(short_code = %error.short_code(), error = %error, "failed to buffer click increment");
    }
}

fn redirect_response(state: &RedirectState, url: &ShortenedUrl) -> Response {
    match location_header(url) {
        Some(location) => (state.redirect_status, [(LOCATION, location)]).into_response(),
        None => internal_error(),
    }
}

fn timed_redirect_response(
    state: &RedirectState,
    url: &ShortenedUrl,
    metadata: LookupMetadata,
    handler_start: Instant,
    request_start: Instant,
) -> Response {
    let location = match location_header(url) {
        Some(location) => location,
        None => return internal_error(),
    };
    let cache_time_ms = metadata
        .cache_duration
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let db_time_ms = metadata
        .db_duration
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, location);
    headers.insert(
        "x-lynx-cache-hit",
        HeaderValue::from_static(if metadata.cache_hit { "true" } else { "false" }),
    );
    headers.insert(
        "x-lynx-timing-total-ms",
        HeaderValue::from(request_start.elapsed().as_millis() as u64),
    );
    headers.insert("x-lynx-timing-cache-ms", HeaderValue::from(cache_time_ms));
    headers.insert("x-lynx-timing-db-ms", HeaderValue::from(db_time_ms));
    headers.insert(
        "x-lynx-timing-handler-ms",
        HeaderValue::from(handler_start.elapsed().as_millis() as u64),
    );
    (state.redirect_status, headers).into_response()
}

fn location_header(url: &ShortenedUrl) -> Option<HeaderValue> {
    match HeaderValue::try_from(&url.original_url) {
        Ok(location) => Some(location),
        Err(error) => {
            tracing::error!(
                short_code = %url.short_code,
                url = %url.original_url,
                error = %error,
                "Failed to create Location header - URL contains invalid characters"
            );
            None
        }
    }
}

fn internal_error() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
}

fn record_analytics(
    short_code: &str,
    headers: &HeaderMap,
    socket_ip: std::net::IpAddr,
    config: &AnalyticsConfig,
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
