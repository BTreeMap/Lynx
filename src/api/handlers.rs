use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::models::{CreateUrlRequest, DeactivateUrlRequest, ShortenedUrl};
use crate::storage::Storage;

pub struct AppState {
    pub storage: Arc<dyn Storage>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// Generate a random short code
fn generate_short_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let num: u64 = rng.gen_range(100000000..9999999999);
    base62::encode(num)
}

/// Create a new shortened URL
pub async fn create_url(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUrlRequest>,
) -> Result<(StatusCode, Json<ShortenedUrl>), (StatusCode, Json<ErrorResponse>)> {
    // Validate URL
    if payload.url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "URL cannot be empty".to_string(),
            }),
        ));
    }

    // Use custom code or generate one
    let short_code = if let Some(custom) = payload.custom_code {
        // Validate custom code
        if custom.is_empty() || custom.len() > 20 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Custom code must be 1-20 characters".to_string(),
                }),
            ));
        }
        
        // Check if already exists
        if state.storage.exists(&custom).await.unwrap_or(false) {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "Short code already exists".to_string(),
                }),
            ));
        }
        
        custom
    } else {
        // Generate unique short code
        let mut code = generate_short_code();
        let mut attempts = 0;
        while state.storage.exists(&code).await.unwrap_or(false) && attempts < 10 {
            code = generate_short_code();
            attempts += 1;
        }
        
        if attempts >= 10 {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate unique short code".to_string(),
                }),
            ));
        }
        
        code
    };

    // Create the shortened URL
    match state
        .storage
        .create(&short_code, &payload.url, None)
        .await
    {
        Ok(url) => Ok((StatusCode::CREATED, Json(url))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create URL: {}", e),
            }),
        )),
    }
}

/// Get a shortened URL by code
pub async fn get_url(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<Json<ShortenedUrl>, (StatusCode, Json<ErrorResponse>)> {
    match state.storage.get(&code).await {
        Ok(Some(url)) => Ok(Json(url)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "URL not found".to_string(),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get URL: {}", e),
            }),
        )),
    }
}

/// Deactivate a shortened URL
pub async fn deactivate_url(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
    Json(_payload): Json<DeactivateUrlRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state.storage.deactivate(&code).await {
        Ok(true) => Ok(Json(SuccessResponse {
            message: "URL deactivated successfully".to_string(),
        })),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "URL not found".to_string(),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to deactivate URL: {}", e),
            }),
        )),
    }
}

/// Reactivate a shortened URL
pub async fn reactivate_url(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state.storage.reactivate(&code).await {
        Ok(true) => Ok(Json(SuccessResponse {
            message: "URL reactivated successfully".to_string(),
        })),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "URL not found".to_string(),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to reactivate URL: {}", e),
            }),
        )),
    }
}

/// List all shortened URLs
pub async fn list_urls(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<ShortenedUrl>>, (StatusCode, Json<ErrorResponse>)> {
    match state.storage.list(query.limit, query.offset).await {
        Ok(urls) => Ok(Json(urls)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to list URLs: {}", e),
            }),
        )),
    }
}

/// Health check endpoint
pub async fn health_check() -> Json<SuccessResponse> {
    Json(SuccessResponse {
        message: "OK".to_string(),
    })
}
