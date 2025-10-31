use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::auth::{auth_middleware, AuthService};
use crate::config::Config;
use crate::storage::Storage;

use super::handlers::{
    create_url, deactivate_url, get_auth_mode, get_url, get_user_info, health_check, list_urls,
    reactivate_url, AppState,
};
use super::static_files::serve_static;
use super::analytics::{get_analytics, get_analytics_aggregate, AnalyticsState};

pub fn create_api_router(
    storage: Arc<dyn Storage>,
    auth_service: Arc<AuthService>,
    config: Arc<Config>,
    analytics_aggregator: Option<Arc<crate::analytics::AnalyticsAggregator>>,
) -> Router {
    let frontend_config = config.frontend.clone();
    let state = Arc::new(AppState { storage: Arc::clone(&storage), config });

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let auth_service_clone1 = Arc::clone(&auth_service);
    let protected_routes = Router::new()
        .route("/urls", post(create_url))
        .route("/urls", get(list_urls))
        .route("/urls/{code}", get(get_url))
        .route("/urls/{code}/deactivate", put(deactivate_url))
        .route("/urls/{code}/reactivate", put(reactivate_url))
        .route("/user/info", get(get_user_info))
        .route_layer(middleware::from_fn(move |headers, req, next| {
            let auth = Arc::clone(&auth_service_clone1);
            auth_middleware(auth, headers, req, next)
        }))
        .with_state(Arc::clone(&state));
    
    // Analytics routes (also protected)
    let analytics_state = Arc::new(AnalyticsState {
        storage: Arc::clone(&storage),
        aggregator: analytics_aggregator,
    });
    let auth_service_clone2 = Arc::clone(&auth_service);
    let analytics_routes = Router::new()
        .route("/analytics/{code}", get(get_analytics))
        .route("/analytics/{code}/aggregate", get(get_analytics_aggregate))
        .route_layer(middleware::from_fn(move |headers, req, next| {
            let auth = Arc::clone(&auth_service_clone2);
            auth_middleware(auth, headers, req, next)
        }))
        .with_state(analytics_state);

    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/auth/mode", get(get_auth_mode))
        .merge(protected_routes)
        .merge(analytics_routes)
        .with_state(Arc::clone(&state))
        .layer(cors);

    // Add frontend static file serving
    let static_dir = frontend_config.static_dir.clone();
    Router::new()
        .nest("/api", api_routes)
        .fallback(move |uri| serve_static(uri, static_dir.clone()))
}
