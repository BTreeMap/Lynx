use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub api_server: ServerConfig,
    pub redirect_server: ServerConfig,
    pub auth: AuthConfig,
    pub frontend: FrontendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub backend: DatabaseBackend,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseBackend {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    None,
    Oauth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub mode: AuthMode,
    #[serde(default)]
    pub oauth: Option<OAuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub issuer_url: String,
    pub audience: String,
    #[serde(default)]
    pub jwks_url: Option<String>,
    #[serde(default = "OAuthConfig::default_cache_ttl_secs")]
    pub jwks_cache_ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfig {
    /// Path to directory containing static frontend files
    /// If None, uses embedded frontend (if available)
    pub static_dir: Option<String>,
}

impl OAuthConfig {
    const fn default_cache_ttl_secs() -> u64 {
        300
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let backend_str =
            std::env::var("DATABASE_BACKEND").unwrap_or_else(|_| "sqlite".to_string());

        let backend = match backend_str.to_lowercase().as_str() {
            "postgres" | "postgresql" => DatabaseBackend::Postgres,
            _ => DatabaseBackend::Sqlite,
        };

        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://./lynx.db".to_string());

        let api_host = std::env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let api_port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()?;

        let redirect_host =
            std::env::var("REDIRECT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let redirect_port = std::env::var("REDIRECT_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()?;

        let disable_auth = std::env::var("DISABLE_AUTH")
            .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false);

        let mut auth_mode = std::env::var("AUTH_MODE")
            .unwrap_or_else(|_| "none".to_string())
            .to_lowercase();

        if disable_auth {
            auth_mode = "none".to_string();
        }

        let auth_mode = match auth_mode.as_str() {
            "none" => AuthMode::None,
            "oauth" => AuthMode::Oauth,
            other => {
                tracing::warn!(
                    "Unknown AUTH_MODE '{other}', falling back to 'none'. Supported values: none, oauth"
                );
                AuthMode::None
            }
        };

        let oauth = if matches!(auth_mode, AuthMode::Oauth) {
            let issuer_url = std::env::var("OAUTH_ISSUER_URL")
                .context("OAUTH_ISSUER_URL must be set when AUTH_MODE=oauth")?;
            let audience = std::env::var("OAUTH_AUDIENCE")
                .context("OAUTH_AUDIENCE must be set when AUTH_MODE=oauth")?;
            let jwks_url = std::env::var("OAUTH_JWKS_URL").ok();
            let jwks_cache_ttl_secs = std::env::var("OAUTH_JWKS_CACHE_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_else(OAuthConfig::default_cache_ttl_secs);

            Some(OAuthConfig {
                issuer_url,
                audience,
                jwks_url,
                jwks_cache_ttl_secs,
            })
        } else {
            None
        };

        let frontend_static_dir = std::env::var("FRONTEND_STATIC_DIR").ok();

        Ok(Config {
            database: DatabaseConfig {
                backend,
                url: database_url,
            },
            api_server: ServerConfig {
                host: api_host,
                port: api_port,
            },
            redirect_server: ServerConfig {
                host: redirect_host,
                port: redirect_port,
            },
            auth: AuthConfig {
                mode: auth_mode,
                oauth,
            },
            frontend: FrontendConfig {
                static_dir: frontend_static_dir,
            },
        })
    }
}
