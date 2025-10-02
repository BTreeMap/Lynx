use axum::{
    extract::Request,
    http::{StatusCode, HeaderMap},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub struct AuthService {
    enabled: bool,
    api_keys: Arc<Vec<String>>,
}

impl AuthService {
    pub fn new(enabled: bool, api_keys: Vec<String>) -> Self {
        Self {
            enabled,
            api_keys: Arc::new(api_keys),
        }
    }

    pub fn validate_key(&self, key: &str) -> bool {
        // If authentication is disabled, allow all requests
        if !self.enabled {
            return true;
        }
        
        // If no API keys configured but auth is enabled, allow all (dev mode)
        if self.api_keys.is_empty() {
            return true;
        }
        
        self.api_keys.iter().any(|k| k == key)
    }
}

pub async fn auth_middleware(
    auth_service: Arc<AuthService>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let api_key = headers
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if auth_service.validate_key(api_key) {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, "Invalid or missing API key").into_response()
    }
}
