mod cloudflare;
mod oauth;

use std::sync::Arc;

use axum::{
    extract::Request,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::Value;
use thiserror::Error;
use tracing::warn;

use crate::config::{AuthConfig, AuthMode};

use self::cloudflare::CloudflareValidator;
use self::oauth::OAuthValidator;

pub struct AuthService {
    strategy: AuthStrategy,
}

enum AuthStrategy {
    None,
    OAuth(Arc<OAuthValidator>),
    Cloudflare(Arc<CloudflareValidator>),
}

#[derive(Clone, Debug)]
pub struct AuthClaims(pub Arc<Value>);

impl std::ops::Deref for AuthClaims {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AuthClaims {
    /// Extract user ID from claims
    /// For Cloudflare and OAuth: uses 'sub' as the unique identifier
    /// For auth=none: returns the legacy user ID
    pub fn user_id(&self) -> Option<String> {
        self.0
            .get("sub")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Get user's email if available
    pub fn email(&self) -> Option<String> {
        self.0
            .get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Check if user has admin role
    /// Checks 'roles' array or 'role' field for 'admin'
    /// Also checks the 'is_admin' boolean field
    pub fn is_admin(&self) -> bool {
        // Check is_admin boolean field first
        if let Some(is_admin) = self.0.get("is_admin").and_then(|v| v.as_bool()) {
            return is_admin;
        }
        
        // Check roles array
        if let Some(roles) = self.0.get("roles").and_then(|v| v.as_array()) {
            return roles.iter().any(|r| {
                r.as_str()
                    .map(|s| s.eq_ignore_ascii_case("admin"))
                    .unwrap_or(false)
            });
        }
        
        // Check single role field
        if let Some(role) = self.0.get("role").and_then(|v| v.as_str()) {
            return role.eq_ignore_ascii_case("admin");
        }
        
        false
    }

    /// Get the authentication method used
    pub fn auth_method(&self) -> Option<String> {
        self.0
            .get("auth_method")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("missing Authorization header")]
    MissingAuthorization,
    #[error("invalid Authorization header format")]
    InvalidAuthorization,
    #[error("authentication misconfiguration: {0}")]
    Misconfigured(String),
    #[error("token validation failed: {0}")]
    Token(String),
}

impl AuthError {
    fn status(&self) -> StatusCode {
        match self {
            AuthError::MissingAuthorization
            | AuthError::InvalidAuthorization
            | AuthError::Token(_) => StatusCode::UNAUTHORIZED,
            AuthError::Misconfigured(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl AuthService {
    pub async fn new(config: AuthConfig) -> anyhow::Result<Self> {
        let strategy = match config.mode {
            AuthMode::None => AuthStrategy::None,
            AuthMode::Oauth => {
                let oauth_config = config.oauth.ok_or_else(|| {
                    anyhow::anyhow!("AUTH_MODE=oauth but no OAuth configuration was provided")
                })?;
                let validator = OAuthValidator::from_config(&oauth_config).await?;
                AuthStrategy::OAuth(Arc::new(validator))
            }
            AuthMode::Cloudflare => {
                let cloudflare_config = config.cloudflare.ok_or_else(|| {
                    anyhow::anyhow!("AUTH_MODE=cloudflare but no Cloudflare configuration was provided")
                })?;
                let validator = CloudflareValidator::from_config(&cloudflare_config).await?;
                AuthStrategy::Cloudflare(Arc::new(validator))
            }
        };

        Ok(Self { strategy })
    }

    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<Option<AuthClaims>, AuthError> {
        match &self.strategy {
            AuthStrategy::None => {
                // For auth=none, return a special admin user with legacy UUID
                let mut claims = serde_json::Map::new();
                claims.insert("sub".to_string(), Value::String("00000000-0000-0000-0000-000000000000".to_string()));
                claims.insert("email".to_string(), Value::String("legacy@nonexistent.joefang.org".to_string()));
                claims.insert("is_admin".to_string(), Value::Bool(true));
                claims.insert("auth_method".to_string(), Value::String("none".to_string()));
                Ok(Some(AuthClaims(Arc::new(Value::Object(claims)))))
            }
            AuthStrategy::OAuth(validator) => {
                let header_value = headers
                    .get(AUTHORIZATION)
                    .ok_or(AuthError::MissingAuthorization)?
                    .to_str()
                    .map_err(|_| AuthError::InvalidAuthorization)?;

                let token = header_value
                    .strip_prefix("Bearer ")
                    .ok_or(AuthError::InvalidAuthorization)?;

                let mut claims = validator
                    .validate(token)
                    .await
                    .map_err(|err| AuthError::Token(err.to_string()))?;

                // Add auth_method to claims
                if let Some(obj) = claims.as_object_mut() {
                    obj.insert("auth_method".to_string(), Value::String("oauth".to_string()));
                }

                Ok(Some(AuthClaims(Arc::new(claims))))
            }
            AuthStrategy::Cloudflare(validator) => {
                // For Cloudflare, check the Cf-Access-Jwt-Assertion header
                let token = headers
                    .get("Cf-Access-Jwt-Assertion")
                    .ok_or(AuthError::MissingAuthorization)?
                    .to_str()
                    .map_err(|_| AuthError::InvalidAuthorization)?;

                let mut claims = validator
                    .validate(token)
                    .await
                    .map_err(|err| AuthError::Token(err.to_string()))?;

                // Add auth_method to claims
                if let Some(obj) = claims.as_object_mut() {
                    obj.insert("auth_method".to_string(), Value::String("cloudflare".to_string()));
                }

                Ok(Some(AuthClaims(Arc::new(claims))))
            }
        }
    }
}

pub async fn auth_middleware(
    auth_service: Arc<AuthService>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    match auth_service.authenticate(&headers).await {
        Ok(Some(claims)) => {
            request.extensions_mut().insert(Some(claims));
            next.run(request).await
        }
        Ok(None) => {
            // This should not happen anymore, but handle it gracefully
            request.extensions_mut().insert(None::<AuthClaims>);
            next.run(request).await
        }
        Err(err) => {
            warn!(error = %err, "Authentication failed");
            (err.status(), err.to_string()).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pass_through_mode_allows_requests() {
        let config = AuthConfig {
            mode: AuthMode::None,
            oauth: None,
            cloudflare: None,
        };
        let service = AuthService::new(config).await.unwrap();
        let headers = HeaderMap::new();
        let result = service.authenticate(&headers).await;
        assert!(result.is_ok());
        let claims = result.unwrap();
        assert!(claims.is_some());
        
        // Verify auth=none creates admin user with legacy UUID
        let claims = claims.unwrap();
        assert_eq!(claims.user_id().unwrap(), "00000000-0000-0000-0000-000000000000");
        assert_eq!(claims.email().unwrap(), "legacy@nonexistent.joefang.org");
        assert!(claims.is_admin());
        assert_eq!(claims.auth_method().unwrap(), "none");
    }
}
