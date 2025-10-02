mod api;
mod auth;
mod config;
mod models;
mod redirect;
mod storage;

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use auth::AuthService;
use config::{Config, DatabaseBackend};
use storage::{PostgresStorage, SqliteStorage, Storage};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Config::from_env()?;
    info!("Loaded configuration");

    // Initialize storage
    let storage: Arc<dyn Storage> = match config.database.backend {
        DatabaseBackend::Sqlite => {
            info!("Using SQLite storage: {}", config.database.url);
            Arc::new(SqliteStorage::new(&config.database.url).await?)
        }
        DatabaseBackend::Postgres => {
            info!("Using PostgreSQL storage: {}", config.database.url);
            Arc::new(PostgresStorage::new(&config.database.url).await?)
        }
    };

    // Initialize database
    info!("Initializing database...");
    storage.init().await?;
    info!("Database initialized successfully");

    // Initialize auth service
    let auth_service = Arc::new(AuthService::new(config.auth.api_keys.clone()));
    if config.auth.api_keys.is_empty() {
        info!("⚠️  Running in development mode - no API keys required");
    } else {
        info!("API keys configured: {} key(s)", config.auth.api_keys.len());
    }

    // Create routers
    let api_router = api::create_api_router(Arc::clone(&storage), auth_service);
    let redirect_router = redirect::create_redirect_router(Arc::clone(&storage));

    // Start API server
    let api_addr = format!("{}:{}", config.api_server.host, config.api_server.port);
    let api_listener = tokio::net::TcpListener::bind(&api_addr).await?;
    info!("🚀 API server listening on http://{}", api_addr);

    // Start redirect server
    let redirect_addr = format!(
        "{}:{}",
        config.redirect_server.host, config.redirect_server.port
    );
    let redirect_listener = tokio::net::TcpListener::bind(&redirect_addr).await?;
    info!("🚀 Redirect server listening on http://{}", redirect_addr);

    // Run both servers concurrently
    tokio::try_join!(
        axum::serve(api_listener, api_router),
        axum::serve(redirect_listener, redirect_router),
    )?;

    Ok(())
}

