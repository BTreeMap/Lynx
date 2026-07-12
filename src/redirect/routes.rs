use axum::{middleware, routing::get, Router};
use std::sync::Arc;

use crate::storage::CachedStorage;
use axum::http::StatusCode;

use super::handlers::{
    health_check, redirect_url, redirect_url_with_analytics,
    redirect_url_with_analytics_and_timing, redirect_url_with_timing, RedirectAnalytics,
    RedirectState,
};
use super::middleware::record_request_start;

pub fn create_redirect_router(
    storage: Arc<CachedStorage>,
    analytics: Option<RedirectAnalytics>,
    enable_timing_headers: bool,
    redirect_status: StatusCode,
) -> Router {
    let analytics_enabled = analytics.is_some();
    let state = Arc::new(RedirectState {
        storage,
        analytics,
        redirect_status,
    });

    let redirect_route = match (analytics_enabled, enable_timing_headers) {
        (false, false) => get(redirect_url),
        (true, false) => get(redirect_url_with_analytics),
        (false, true) => {
            get(redirect_url_with_timing).layer(middleware::from_fn(record_request_start))
        }
        (true, true) => get(redirect_url_with_analytics_and_timing)
            .layer(middleware::from_fn(record_request_start)),
    };

    Router::new()
        .route("/", get(health_check))
        .route("/{*code}", redirect_route)
        .with_state(state)
}
