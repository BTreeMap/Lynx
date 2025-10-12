use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use anyhow::anyhow;
use rand::distr::{Alphanumeric, Distribution};

use crate::auth::AuthClaims;
use crate::config::Config;
use crate::models::{CreateUrlRequest, DeactivateUrlRequest, ShortenedUrl};
use crate::storage::{Storage, StorageError};

pub struct AppState {
    pub storage: Arc<dyn Storage>,
    pub config: Arc<Config>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

#[derive(Serialize)]
pub struct ShortenedUrlResponse {
    #[serde(flatten)]
    pub inner: ShortenedUrl,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_base_url: Option<String>,
}

impl ShortenedUrlResponse {
    fn with_base(url: ShortenedUrl, base: Option<&str>) -> Self {
        Self {
            inner: url,
            redirect_base_url: base.map(|value| value.to_owned()),
        }
    }
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

/// Helper to check if user is admin (combines JWT claims and manual promotion)
/// JWT claims take precedence - if JWT says admin, they're admin regardless of manual table
/// Manual promotion only applies when JWT doesn't grant admin status
async fn is_user_admin(storage: &dyn Storage, claims: &Option<AuthClaims>) -> bool {
    if let Some(c) = claims {
        // First check JWT claims - these take precedence
        if c.is_admin() {
            return true;
        }

        // Only check manual promotion if JWT doesn't grant admin
        if let (Some(user_id), Some(auth_method)) = (c.user_id(), c.auth_method()) {
            if let Ok(manual_admin) = storage.is_manual_admin(&user_id, &auth_method).await {
                return manual_admin;
            }
        }
    }

    false
}

const MIN_SHORT_CODE_LENGTH: usize = 3;
const MAX_SHORT_CODE_LENGTH: usize = 10;
const MIN_PROBES_BEFORE_ESCALATION: usize = 5;
const MAX_PROBES_PER_LENGTH: usize = 64;
/// Precomputed minimum number of successes required after each attempt
/// to keep the expected number of probes below the target threshold.
/// Generated in `build.rs` to avoid runtime statistical calculations.
const REQUIRED_SUCCESSES: [u8; MAX_PROBES_PER_LENGTH] =
    include!(concat!(env!("OUT_DIR"), "/required_successes.in"));

fn random_code(length: usize) -> String {
    let mut rng = rand::rng();
    (0..length)
        .map(|_| Alphanumeric.sample(&mut rng) as char)
        .collect()
}

async fn create_with_random_code(
    storage: &dyn Storage,
    original_url: &str,
    created_by: Option<&str>,
) -> Result<ShortenedUrl, StorageError> {
    for length in MIN_SHORT_CODE_LENGTH..=MAX_SHORT_CODE_LENGTH {
        let mut attempts = 0usize;
        let mut failures = 0usize;

        while attempts < MAX_PROBES_PER_LENGTH {
            let candidate = random_code(length);
            attempts += 1;

            match storage
                .create_with_code(&candidate, original_url, created_by)
                .await
            {
                Ok(url) => return Ok(url),
                Err(StorageError::Conflict) => {
                    failures += 1;
                    if attempts >= MIN_PROBES_BEFORE_ESCALATION {
                        let successes = attempts - failures;
                        let required = REQUIRED_SUCCESSES[attempts - 1];
                        if required == u8::MAX || successes < required as usize {
                            break;
                        }
                    }
                }
                Err(StorageError::Other(e)) => return Err(StorageError::Other(e)),
            }
        }
    }

    Err(StorageError::Other(anyhow!(
        "Failed to generate unique short code"
    )))
}

/// Create a new shortened URL
pub async fn create_url(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Option<AuthClaims>>,
    Json(payload): Json<CreateUrlRequest>,
) -> Result<(StatusCode, Json<ShortenedUrlResponse>), (StatusCode, Json<ErrorResponse>)> {
    let base = Some(state.config.redirect_base_url.as_str());

    let CreateUrlRequest { url, custom_code } = payload;

    if url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "URL cannot be empty".to_string(),
            }),
        ));
    }

    // Extract user ID from claims
    let created_by = claims.as_ref().and_then(|c| c.user_id());
    let created_by_ref = created_by.as_deref();

    let created = if let Some(custom) = custom_code {
        if custom.is_empty() || custom.len() > 20 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Custom code must be 1-20 characters".to_string(),
                }),
            ));
        }

        match state
            .storage
            .create_with_code(&custom, &url, created_by_ref)
            .await
        {
            Ok(url) => Ok((
                StatusCode::CREATED,
                Json(ShortenedUrlResponse::with_base(url, base)),
            )),
            Err(StorageError::Conflict) => Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "Short code already exists".to_string(),
                }),
            )),
            Err(StorageError::Other(e)) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create URL with custom code: {}", e),
                }),
            )),
        }
    } else {
        match create_with_random_code(state.storage.as_ref(), &url, created_by_ref).await {
            Ok(url) => Ok((
                StatusCode::CREATED,
                Json(ShortenedUrlResponse::with_base(url, base)),
            )),
            Err(e) => match e {
                StorageError::Conflict => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to generate unique short code after multiple attempts"
                            .to_string(),
                    }),
                )),
                StorageError::Other(err) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to create URL: {}", err),
                    }),
                )),
            },
        }
    };

    created
}

/// Get a shortened URL by code
pub async fn get_url(
    State(state): State<Arc<AppState>>,
    Path(code): Path<String>,
) -> Result<Json<ShortenedUrlResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state.storage.get(&code).await {
        Ok(Some(url)) => Ok(Json(ShortenedUrlResponse::with_base(
            url,
            Some(state.config.redirect_base_url.as_str()),
        ))),
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

/// Deactivate a shortened URL (admin only)
pub async fn deactivate_url(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Option<AuthClaims>>,
    Path(code): Path<String>,
    Json(_payload): Json<DeactivateUrlRequest>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if user is admin
    let is_admin = is_user_admin(state.storage.as_ref(), &claims).await;
    if !is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Only administrators can deactivate URLs".to_string(),
            }),
        ));
    }

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

/// Reactivate a shortened URL (admin only)
pub async fn reactivate_url(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Option<AuthClaims>>,
    Path(code): Path<String>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if user is admin
    let is_admin = is_user_admin(state.storage.as_ref(), &claims).await;
    if !is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Only administrators can reactivate URLs".to_string(),
            }),
        ));
    }

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
    Extension(claims): Extension<Option<AuthClaims>>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<ShortenedUrlResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let is_admin = is_user_admin(state.storage.as_ref(), &claims).await;
    let user_id = claims.as_ref().and_then(|c| c.user_id());

    match state
        .storage
        .list(query.limit, query.offset, is_admin, user_id.as_deref())
        .await
    {
        Ok(urls) => {
            let base = Some(state.config.redirect_base_url.as_str());
            let response = urls
                .into_iter()
                .map(|url| ShortenedUrlResponse::with_base(url, base))
                .collect();
            Ok(Json(response))
        }
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

#[derive(Serialize)]
pub struct UserInfo {
    pub user_id: Option<String>,
    pub is_admin: bool,
}

/// Get current user information from token
pub async fn get_user_info(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Option<AuthClaims>>,
) -> Json<UserInfo> {
    let user_id = claims.as_ref().and_then(|c| c.user_id());
    let is_admin = is_user_admin(state.storage.as_ref(), &claims).await;

    // Upsert user metadata if authenticated
    if let (Some(ref uid), Some(ref c)) = (&user_id, &claims) {
        if let Some(auth_method) = c.auth_method() {
            let email = c.email();
            let _ = state
                .storage
                .upsert_user(uid, email.as_deref(), &auth_method)
                .await;
        }
    }

    Json(UserInfo { user_id, is_admin })
}

#[derive(Serialize)]
pub struct AuthModeResponse {
    pub mode: String,
}

/// Get the authentication mode configured for this instance
/// This endpoint is public (no auth required) so frontend can determine auth flow
pub async fn get_auth_mode(State(state): State<Arc<AppState>>) -> Json<AuthModeResponse> {
    let mode = match state.config.auth.mode {
        crate::config::AuthMode::None => "none",
        crate::config::AuthMode::Oauth => "oauth",
        crate::config::AuthMode::Cloudflare => "cloudflare",
    };

    Json(AuthModeResponse {
        mode: mode.to_string(),
    })
}
