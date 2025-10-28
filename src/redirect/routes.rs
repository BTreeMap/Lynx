use axum::{middleware, routing::get, Router};
use std::sync::Arc;

use crate::config::AnalyticsConfig;
use crate::storage::Storage;
use crate::analytics::{AnalyticsAggregator, GeoIpService};

use super::handlers::{health_check, redirect_url, RedirectState};
use super::middleware::record_request_start;

pub fn create_redirect_router(
    storage: Arc<dyn Storage>,
    analytics_config: Option<AnalyticsConfig>,
    geoip_service: Option<Arc<GeoIpService>>,
    analytics_aggregator: Option<Arc<AnalyticsAggregator>>,
) -> Router {
    let state = Arc::new(RedirectState {
        storage,
        analytics_config,
        geoip_service,
        analytics_aggregator,
    });

    Router::new()
        .route("/", get(health_check))
        .route("/{code}", get(redirect_url))
        .layer(middleware::from_fn(record_request_start))
        .with_state(state)
}
