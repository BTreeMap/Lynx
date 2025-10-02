use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub api_server: ServerConfig,
    pub redirect_server: ServerConfig,
    pub auth: AuthConfig,
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
pub struct AuthConfig {
    pub enabled: bool,
    pub api_keys: Vec<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        
        let backend_str = std::env::var("DATABASE_BACKEND")
            .unwrap_or_else(|_| "sqlite".to_string());
        
        let backend = match backend_str.to_lowercase().as_str() {
            "postgres" | "postgresql" => DatabaseBackend::Postgres,
            _ => DatabaseBackend::Sqlite,
        };

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://./lynx.db".to_string());

        let api_host = std::env::var("API_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string());
        let api_port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()?;

        let redirect_host = std::env::var("REDIRECT_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string());
        let redirect_port = std::env::var("REDIRECT_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()?;

        // Check if authentication is disabled
        let auth_enabled = std::env::var("DISABLE_AUTH")
            .map(|v| v.to_lowercase() != "true")
            .unwrap_or(true);

        let api_keys_str = std::env::var("API_KEYS")
            .unwrap_or_else(|_| String::new());
        let api_keys: Vec<String> = api_keys_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();

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
                enabled: auth_enabled,
                api_keys 
            },
        })
    }
}
