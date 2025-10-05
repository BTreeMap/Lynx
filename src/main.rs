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
use config::{AuthMode, Config, DatabaseBackend};
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
    let auth_config = config.auth.clone();
    let auth_service = Arc::new(AuthService::new(auth_config.clone()).await?);

    match auth_config.mode {
        AuthMode::None => {
            info!("🔓 Authentication is disabled - all API requests are allowed");
        }
        AuthMode::Oauth => {
            if let Some(oauth) = auth_config.oauth.as_ref() {
                info!(
                    "🔐 OAuth authentication enabled (issuer: {}, audience: {})",
                    oauth.issuer_url, oauth.audience
                );
            } else {
                info!("🔐 OAuth authentication enabled");
            }
        }
    }

    // Create routers
    let api_router = api::create_api_router(
        Arc::clone(&storage),
        auth_service,
        config.frontend.clone(),
    );
    let redirect_router = redirect::create_redirect_router(Arc::clone(&storage));

    // Log frontend configuration
    if let Some(ref static_dir) = config.frontend.static_dir {
        info!("🎨 Serving frontend from directory: {}", static_dir);
    } else {
        info!("🎨 Serving embedded frontend");
    }

    // Start API server
    let api_addr = format!("{}:{}", config.api_server.host, config.api_server.port);
    let api_listener = tokio::net::TcpListener::bind(&api_addr).await?;
    info!("🚀 API server listening on http://{}", api_addr);
    info!("   - API endpoints available at http://{}/api/...", api_addr);
    info!("   - Frontend UI available at http://{}/", api_addr);

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
