use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::auth::{auth_middleware, AuthService};
use crate::config::FrontendConfig;
use crate::storage::Storage;

use super::handlers::{
    create_url, deactivate_url, get_url, get_user_info, health_check, list_urls, reactivate_url,
    AppState,
};
use super::static_files::serve_static;

pub fn create_api_router(
    storage: Arc<dyn Storage>,
    auth_service: Arc<AuthService>,
    frontend_config: FrontendConfig,
) -> Router {
    let state = Arc::new(AppState { storage });

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let protected_routes = Router::new()
        .route("/urls", post(create_url))
        .route("/urls", get(list_urls))
        .route("/urls/{code}", get(get_url))
        .route("/urls/{code}/deactivate", put(deactivate_url))
        .route("/urls/{code}/reactivate", put(reactivate_url))
        .route("/user/info", get(get_user_info))
        .route_layer(middleware::from_fn(move |headers, req, next| {
            let auth = Arc::clone(&auth_service);
            auth_middleware(auth, headers, req, next)
        }))
        .with_state(Arc::clone(&state));

    let api_routes = Router::new()
        .route("/health", get(health_check))
        .merge(protected_routes)
        .layer(cors);

    // Add frontend static file serving
    let static_dir = frontend_config.static_dir.clone();
    Router::new()
        .nest("/api", api_routes)
        .fallback(move |uri| serve_static(uri, static_dir.clone()))
}
