mod api;
mod auth;
mod config;
mod models;
mod redirect;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::info;

use auth::AuthService;
use config::{AuthMode, Config, DatabaseBackend};
use storage::{CachedStorage, PostgresStorage, SqliteStorage, Storage};

#[derive(Parser)]
#[command(name = "lynx")]
#[command(about = "Lynx URL Shortener", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Admin management commands
    Admin {
        #[command(subcommand)]
        admin_command: AdminCommands,
    },
}

#[derive(Subcommand)]
enum AdminCommands {
    /// Promote a user to admin
    Promote {
        /// User ID (sub claim from JWT)
        user_id: String,
        /// Authentication method (oauth, cloudflare)
        auth_method: String,
    },
    /// Demote a user from admin
    Demote {
        /// User ID (sub claim from JWT)
        user_id: String,
        /// Authentication method (oauth, cloudflare)
        auth_method: String,
    },
    /// List all manually promoted admins
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Handle admin commands
    if let Some(Commands::Admin { admin_command }) = cli.command {
        return handle_admin_command(admin_command).await;
    }

    // Otherwise, run the server
    run_server().await
}

async fn handle_admin_command(command: AdminCommands) -> Result<()> {
    let config = Config::from_env()?;
    
    let storage: Arc<dyn Storage> = match config.database.backend {
        DatabaseBackend::Sqlite => {
            Arc::new(SqliteStorage::new(&config.database.url, config.database.max_connections).await?)
        }
        DatabaseBackend::Postgres => {
            Arc::new(PostgresStorage::new(&config.database.url, config.database.max_connections).await?)
        }
    };

    // Ensure database is initialized
    storage.init().await?;

    match command {
        AdminCommands::Promote {
            user_id,
            auth_method,
        } => {
            storage.promote_to_admin(&user_id, &auth_method).await?;
            println!(
                "✓ Promoted user '{}' with auth method '{}' to admin",
                user_id, auth_method
            );
        }
        AdminCommands::Demote {
            user_id,
            auth_method,
        } => {
            let demoted = storage.demote_from_admin(&user_id, &auth_method).await?;
            if demoted {
                println!(
                    "✓ Demoted user '{}' with auth method '{}' from admin",
                    user_id, auth_method
                );
            } else {
                println!(
                    "⚠ User '{}' with auth method '{}' was not an admin",
                    user_id, auth_method
                );
            }
        }
        AdminCommands::List => {
            let admins = storage.list_manual_admins().await?;
            if admins.is_empty() {
                println!("No manually promoted admins found.");
            } else {
                println!("Manually promoted admins:");
                println!("{:<40} {:<15} {}", "User ID", "Auth Method", "Email");
                println!("{}", "-".repeat(80));
                for (user_id, auth_method, email) in admins {
                    println!("{:<40} {:<15} {}", user_id, auth_method, email);
                }
            }
        }
    }

    Ok(())
}

async fn run_server() -> Result<()> {

    // Load configuration
    let config = Arc::new(Config::from_env()?);
    info!("Loaded configuration");

    // Initialize storage
    let base_storage: Arc<dyn Storage> = match config.database.backend {
        DatabaseBackend::Sqlite => {
            info!("Using SQLite storage: {}", config.database.url);
            Arc::new(SqliteStorage::new(&config.database.url, config.database.max_connections).await?)
        }
        DatabaseBackend::Postgres => {
            info!("Using PostgreSQL storage: {}", config.database.url);
            Arc::new(PostgresStorage::new(&config.database.url, config.database.max_connections).await?)
        }
    };

    // Initialize database
    info!("Initializing database...");
    base_storage.init().await?;
    info!("Database initialized successfully");

    // Wrap with cached storage for performance
    info!(
        "Initializing cache with max {} entries and DB pool with max {} connections",
        config.cache.max_entries, config.database.max_connections
    );
    let storage: Arc<dyn Storage> = Arc::new(CachedStorage::new(
        base_storage,
        config.cache.max_entries,
    ));

    // Initialize auth service
    let auth_config = config.auth.clone();
    let auth_service = Arc::new(AuthService::new(auth_config.clone()).await?);

    match auth_config.mode {
        AuthMode::None => {
            info!("🔓 Authentication is disabled - all API requests are allowed as admin");
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
        AuthMode::Cloudflare => {
            if let Some(cf) = auth_config.cloudflare.as_ref() {
                info!(
                    "☁️  Cloudflare Zero Trust authentication enabled (team: {}, audience: {})",
                    cf.team_domain, cf.audience
                );
            } else {
                info!("☁️  Cloudflare Zero Trust authentication enabled");
            }
        }
    }

    // Create routers
    info!(
        "🔗 Redirect base URL advertised to clients: {}",
        config.redirect_base_url
    );

    let api_router =
        api::create_api_router(Arc::clone(&storage), auth_service, Arc::clone(&config));
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
    info!(
        "   - API endpoints available at http://{}/api/...",
        api_addr
    );
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
