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

/// Create a new shortened URL
pub async fn create_url(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUrlRequest>,
) -> Result<(StatusCode, Json<ShortenedUrl>), (StatusCode, Json<ErrorResponse>)> {
    let CreateUrlRequest { url, custom_code } = payload;

    if url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "URL cannot be empty".to_string(),
            }),
        ));
    }

    let created = if let Some(custom) = custom_code {
        if custom.is_empty() || custom.len() > 20 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Custom code must be 1-20 characters".to_string(),
                }),
            ));
        }

        match state.storage.exists(&custom).await {
            Ok(true) => Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "Short code already exists".to_string(),
                }),
            )),
            Ok(false) => state
                .storage
                .create_with_code(&custom, &url, None)
                .await
                .map(|url| (StatusCode::CREATED, Json(url)))
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to create URL with custom code: {}", e),
                        }),
                    )
                }),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to validate custom code availability: {}", e),
                }),
            )),
        }
    } else {
        state
            .storage
            .create_auto(&url, None)
            .await
            .map(|url| (StatusCode::CREATED, Json(url)))
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to create URL: {}", e),
                    }),
                )
            })
    };

    created
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
