use axum::{middleware, routing::get, Router};
use std::sync::Arc;

use crate::storage::Storage;

use super::handlers::{health_check, redirect_url, RedirectState};
use super::middleware::record_request_start;

pub fn create_redirect_router(storage: Arc<dyn Storage>) -> Router {
    let state = Arc::new(RedirectState { storage });

    Router::new()
        .route("/", get(health_check))
        .route("/{code}", get(redirect_url))
        .layer(middleware::from_fn(record_request_start))
        .with_state(state)
}
