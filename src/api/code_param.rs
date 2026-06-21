use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use super::handlers::ApiError;

pub fn decode_code_path_param(code: &str) -> Result<String, ApiError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(code)
        .map_err(|_| ApiError::BadRequest("Invalid encoded short code".to_string()))?;

    String::from_utf8(bytes)
        .map_err(|_| ApiError::BadRequest("Invalid encoded short code".to_string()))
}
