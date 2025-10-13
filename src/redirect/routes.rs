use axum::{routing::get, Router};
use std::sync::Arc;

use crate::storage::Storage;

use super::handlers::{health_check, redirect_url, RedirectState};

pub fn create_redirect_router(storage: Arc<dyn Storage>) -> Router {
    let state = Arc::new(RedirectState { storage });

    Router::new()
        .route("/", get(health_check))
        .route("/{code}", get(redirect_url))
        .with_state(state)
}
