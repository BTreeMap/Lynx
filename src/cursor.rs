use anyhow::{anyhow, Result};
use base64::prelude::*;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::OnceLock;

/// Global HMAC key for cursor signing
static HMAC_KEY: OnceLock<Vec<u8>> = OnceLock::new();

/// Initialize the HMAC key for cursor signing
/// If secret is None, generates a random key (WARNING: cursors won't survive restarts)
pub fn init_cursor_hmac_key(secret: Option<&str>) {
    let key = if let Some(s) = secret {
        s.as_bytes().to_vec()
    } else {
        use rand::Rng;
        let mut rng = rand::rng();
        (0..32).map(|_| rng.random::<u8>()).collect()
    };

    HMAC_KEY.get_or_init(|| key);
}

/// Get the HMAC key, initializing with a random key if not already set
fn get_hmac_key() -> &'static [u8] {
    HMAC_KEY.get_or_init(|| {
        use rand::Rng;
        let mut rng = rand::rng();
        (0..32).map(|_| rng.random::<u8>()).collect()
    })
}

/// Cursor data for pagination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorData {
    pub created_at: i64,
    pub id: i64,
}

/// Create a signed cursor from data
pub fn create_cursor(data: &CursorData) -> Result<String> {
    // Serialize the cursor data
    let json = serde_json::to_string(data)?;
    let payload = BASE64_URL_SAFE_NO_PAD.encode(json.as_bytes());

    // Create HMAC signature
    let key = get_hmac_key();
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key).map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;

    mac.update(payload.as_bytes());
    let signature = mac.finalize();
    let signature_bytes = signature.into_bytes();
    let signature_b64 = BASE64_URL_SAFE_NO_PAD.encode(signature_bytes);

    // Return payload.signature
    Ok(format!("{}.{}", payload, signature_b64))
}

/// Verify and decode a cursor
pub fn verify_cursor(cursor: &str) -> Result<CursorData> {
    // Split cursor into payload and signature
    let parts: Vec<&str> = cursor.split('.').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid cursor format"));
    }

    let payload = parts[0];
    let signature_b64 = parts[1];

    // Verify signature
    let key = get_hmac_key();
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key).map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;

    mac.update(payload.as_bytes());

    let expected_signature = mac.finalize();
    let expected_bytes = expected_signature.into_bytes();

    let provided_bytes = BASE64_URL_SAFE_NO_PAD
        .decode(signature_b64)
        .map_err(|_| anyhow!("Invalid cursor signature encoding"))?;

    // Constant-time comparison
    use subtle::ConstantTimeEq;
    if expected_bytes.ct_eq(&provided_bytes[..]).into() {
        // Decode payload
        let json_bytes = BASE64_URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| anyhow!("Invalid cursor payload encoding"))?;
        let json_str =
            std::str::from_utf8(&json_bytes).map_err(|_| anyhow!("Invalid cursor UTF-8"))?;
        let data: CursorData =
            serde_json::from_str(json_str).map_err(|_| anyhow!("Invalid cursor data"))?;
        Ok(data)
    } else {
        Err(anyhow!("Cursor signature verification failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_create_and_verify() {
        // Initialize with a static key for testing
        init_cursor_hmac_key(Some("test_secret_key_for_hmac_signing"));

        let data = CursorData {
            created_at: 1234567890,
            id: 42,
        };

        let cursor = create_cursor(&data).unwrap();
        let verified = verify_cursor(&cursor).unwrap();

        assert_eq!(verified.created_at, data.created_at);
        assert_eq!(verified.id, data.id);
    }

    #[test]
    fn test_cursor_tampering_detection() {
        init_cursor_hmac_key(Some("test_secret_key_for_hmac_signing"));

        let data = CursorData {
            created_at: 1234567890,
            id: 42,
        };

        let cursor = create_cursor(&data).unwrap();

        // Try to tamper with the cursor
        let parts: Vec<&str> = cursor.split('.').collect();
        let tampered = format!("{}.invalid_signature", parts[0]);

        assert!(verify_cursor(&tampered).is_err());
    }

    #[test]
    fn test_cursor_invalid_format() {
        assert!(verify_cursor("invalid").is_err());
        assert!(verify_cursor("invalid.format.extra").is_err());
    }
}
