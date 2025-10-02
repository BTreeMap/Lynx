use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use std::sync::Arc;

use crate::storage::Storage;

pub struct RedirectState {
    pub storage: Arc<dyn Storage>,
}

/// Redirect to original URL
pub async fn redirect_url(
    State(state): State<Arc<RedirectState>>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    // Get the URL
    match state.storage.get(&code).await {
        Ok(Some(url)) => {
            // Check if URL is active
            if !url.is_active {
                return (StatusCode::GONE, "This link has been deactivated").into_response();
            }
            
            // Increment clicks asynchronously (fire and forget)
            let storage = Arc::clone(&state.storage);
            let code_clone = code.clone();
            tokio::spawn(async move {
                let _ = storage.increment_clicks(&code_clone).await;
            });
            
            // Redirect
            Redirect::permanent(&url.original_url).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "URL not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response(),
    }
}

/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}
