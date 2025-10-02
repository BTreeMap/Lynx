use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;

use crate::auth::{auth_middleware, AuthService};
use crate::storage::Storage;

use super::handlers::{create_url, delete_url, get_url, health_check, list_urls, update_url, AppState};

pub fn create_api_router(storage: Arc<dyn Storage>, auth_service: Arc<AuthService>) -> Router {
    let state = Arc::new(AppState { storage });

    let protected_routes = Router::new()
        .route("/urls", post(create_url))
        .route("/urls", get(list_urls))
        .route("/urls/:code", get(get_url))
        .route("/urls/:code", put(update_url))
        .route("/urls/:code", delete(delete_url))
        .route_layer(middleware::from_fn(move |headers, req, next| {
            let auth = Arc::clone(&auth_service);
            auth_middleware(auth, headers, req, next)
        }))
        .with_state(Arc::clone(&state));

    Router::new()
        .route("/health", get(health_check))
        .merge(protected_routes)
}

