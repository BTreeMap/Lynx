use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub api_server: ServerConfig,
    pub redirect_server: ServerConfig,
    pub redirect_base_url: String,
    pub auth: AuthConfig,
    pub frontend: FrontendConfig,
    pub cache: CacheConfig,
    pub pagination: PaginationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "CacheConfig::default_max_entries")]
    pub max_entries: u64,
    #[serde(default = "CacheConfig::default_flush_interval_secs")]
    pub flush_interval_secs: u64,
    #[serde(default = "CacheConfig::default_actor_buffer_size")]
    pub actor_buffer_size: usize,
    #[serde(default = "CacheConfig::default_actor_flush_interval_ms")]
    pub actor_flush_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationConfig {
    /// HMAC secret for cursor signing
    /// If None, a dynamic key is generated at startup (not recommended for production)
    pub cursor_hmac_secret: Option<String>,
}

impl CacheConfig {
    const fn default_max_entries() -> u64 {
        500_000
    }

    const fn default_flush_interval_secs() -> u64 {
        5
    }

    const fn default_actor_buffer_size() -> usize {
        1_000_000
    }

    const fn default_actor_flush_interval_ms() -> u64 {
        100
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub backend: DatabaseBackend,
    pub url: String,
    #[serde(default = "DatabaseConfig::default_max_connections")]
    pub max_connections: u32,
}

impl DatabaseConfig {
    const fn default_max_connections() -> u32 {
        30
    }
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
    Cloudflare,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub mode: AuthMode,
    #[serde(default)]
    pub oauth: Option<OAuthConfig>,
    #[serde(default)]
    pub cloudflare: Option<CloudflareConfig>,
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
pub struct CloudflareConfig {
    pub team_domain: String,
    pub audience: String,
    #[serde(default = "CloudflareConfig::default_cache_ttl_secs")]
    pub certs_cache_ttl_secs: u64,
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

impl CloudflareConfig {
    const fn default_cache_ttl_secs() -> u64 {
        86400 // 24 hours
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

        let database_max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or_else(DatabaseConfig::default_max_connections);

        let cache_max_entries = std::env::var("CACHE_MAX_ENTRIES")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(CacheConfig::default_max_entries);

        let cache_flush_interval_secs = std::env::var("CACHE_FLUSH_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(CacheConfig::default_flush_interval_secs);

        let actor_buffer_size = std::env::var("ACTOR_BUFFER_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(CacheConfig::default_actor_buffer_size);

        let actor_flush_interval_ms = std::env::var("ACTOR_FLUSH_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or_else(CacheConfig::default_actor_flush_interval_ms);

        let api_host = std::env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let api_port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()?;

        let redirect_host =
            std::env::var("REDIRECT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let redirect_port = std::env::var("REDIRECT_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()?;
        let redirect_scheme = std::env::var("REDIRECT_SCHEME")
            .ok()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if redirect_port == 443 {
                    "https"
                } else {
                    "http"
                }
                .to_string()
            });

        let redirect_base_url = std::env::var("REDIRECT_BASE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| match (redirect_scheme.as_str(), redirect_port) {
                ("http", 80) | ("https", 443) => {
                    format!("{}://{}", redirect_scheme, redirect_host)
                }
                _ => format!("{}://{}:{}", redirect_scheme, redirect_host, redirect_port),
            });

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
            "cloudflare" => AuthMode::Cloudflare,
            other => {
                tracing::warn!(
                    "Unknown AUTH_MODE '{other}', falling back to 'none'. Supported values: none, oauth, cloudflare"
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

        let cloudflare = if matches!(auth_mode, AuthMode::Cloudflare) {
            let team_domain = std::env::var("CLOUDFLARE_TEAM_DOMAIN")
                .context("CLOUDFLARE_TEAM_DOMAIN must be set when AUTH_MODE=cloudflare")?;
            let audience = std::env::var("CLOUDFLARE_AUDIENCE")
                .context("CLOUDFLARE_AUDIENCE must be set when AUTH_MODE=cloudflare")?;
            let certs_cache_ttl_secs = std::env::var("CLOUDFLARE_CERTS_CACHE_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or_else(CloudflareConfig::default_cache_ttl_secs);

            Some(CloudflareConfig {
                team_domain,
                audience,
                certs_cache_ttl_secs,
            })
        } else {
            None
        };

        let frontend_static_dir = std::env::var("FRONTEND_STATIC_DIR").ok();

        let cursor_hmac_secret = std::env::var("CURSOR_HMAC_SECRET").ok();

        // Warn if cursor HMAC secret is not set
        if cursor_hmac_secret.is_none() {
            tracing::warn!(
                "CURSOR_HMAC_SECRET is not set. Using a dynamic key generated at runtime. \
                Previous cursors won't work after server restart. \
                For production, set CURSOR_HMAC_SECRET in your environment."
            );
        }

        Ok(Config {
            database: DatabaseConfig {
                backend,
                url: database_url,
                max_connections: database_max_connections,
            },
            api_server: ServerConfig {
                host: api_host,
                port: api_port,
            },
            redirect_server: ServerConfig {
                host: redirect_host,
                port: redirect_port,
            },
            redirect_base_url,
            auth: AuthConfig {
                mode: auth_mode,
                oauth,
                cloudflare,
            },
            frontend: FrontendConfig {
                static_dir: frontend_static_dir,
            },
            cache: CacheConfig {
                max_entries: cache_max_entries,
                flush_interval_secs: cache_flush_interval_secs,
                actor_buffer_size,
                actor_flush_interval_ms,
            },
            pagination: PaginationConfig {
                cursor_hmac_secret,
            },
        })
    }
}
