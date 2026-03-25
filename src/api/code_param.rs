use axum::{http::StatusCode, Json};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use super::handlers::ErrorResponse;

pub fn decode_code_path_param(code: &str) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let bytes = URL_SAFE_NO_PAD.decode(code).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid encoded short code".to_string(),
            }),
        )
    })?;

    String::from_utf8(bytes).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid encoded short code".to_string(),
            }),
        )
    })
}
