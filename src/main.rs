use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::info;

use lynx::auth::AuthService;
use lynx::config::{AuthMode, Config, DatabaseBackend};
use lynx::storage::{CachedStorage, PostgresStorage, SqliteStorage, Storage};

// Type alias for analytics record tuple to reduce complexity
type AnalyticsRecord = (String, i64, Option<String>, Option<String>, Option<String>, Option<i64>, i32, i64);

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
    /// Database patching commands
    Patch {
        #[command(subcommand)]
        patch_command: PatchCommands,
    },
    /// User management commands
    User {
        #[command(subcommand)]
        user_command: UserCommands,
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

#[derive(Subcommand)]
enum PatchCommands {
    /// Patch the created_by field for a specific short link
    Link {
        /// User identifier to set as created_by
        user_id: String,
        /// Short code to patch
        short_code: String,
    },
    /// Fix all malformed created_by values (all-zero UUID or null)
    FixAll {
        /// User identifier to set for all malformed entries
        user_id: String,
    },
}

#[derive(Subcommand)]
enum UserCommands {
    /// List all users
    List {
        /// Number of results per page (default: 50)
        #[arg(short, long, default_value_t = 50)]
        limit: i64,
        /// Page number (starts from 1)
        #[arg(short, long, default_value_t = 1)]
        page: i64,
    },
    /// List all admin users
    ListAdmins,
    /// List all links created by a specific user
    Links {
        /// User ID to list links for
        user_id: String,
        /// Number of results per page (default: 50)
        #[arg(short, long, default_value_t = 50)]
        limit: i64,
        /// Page number (starts from 1)
        #[arg(short, long, default_value_t = 1)]
        page: i64,
    },
    /// Deactivate all links created by a user
    DeactivateLinks {
        /// User ID whose links to deactivate
        user_id: String,
    },
    /// Reactivate all links created by a user
    ReactivateLinks {
        /// User ID whose links to reactivate
        user_id: String,
    },
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

    // Handle patch commands
    if let Some(Commands::Patch { patch_command }) = cli.command {
        return handle_patch_command(patch_command).await;
    }

    // Handle user commands
    if let Some(Commands::User { user_command }) = cli.command {
        return handle_user_command(user_command).await;
    }

    // Otherwise, run the server
    run_server().await
}

async fn handle_admin_command(command: AdminCommands) -> Result<()> {
    let config = Config::from_env()?;

    let storage: Arc<dyn Storage> = match config.database.backend {
        DatabaseBackend::Sqlite => Arc::new(
            SqliteStorage::new(&config.database.url, config.database.max_connections).await?,
        ),
        DatabaseBackend::Postgres => Arc::new(
            PostgresStorage::new(&config.database.url, config.database.max_connections).await?,
        ),
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
                println!("{:<40} {:<15} Email", "User ID", "Auth Method");
                println!("{}", "-".repeat(80));
                for (user_id, auth_method, email) in admins {
                    println!("{:<40} {:<15} {}", user_id, auth_method, email);
                }
            }
        }
    }

    Ok(())
}

async fn handle_patch_command(command: PatchCommands) -> Result<()> {
    let config = Config::from_env()?;

    let storage: Arc<dyn Storage> = match config.database.backend {
        DatabaseBackend::Sqlite => Arc::new(
            SqliteStorage::new(&config.database.url, config.database.max_connections).await?,
        ),
        DatabaseBackend::Postgres => Arc::new(
            PostgresStorage::new(&config.database.url, config.database.max_connections).await?,
        ),
    };

    // Ensure database is initialized
    storage.init().await?;

    match command {
        PatchCommands::Link {
            user_id,
            short_code,
        } => {
            // First verify the short code exists
            let url = storage.get_authoritative(&short_code).await?;
            if url.is_none() {
                println!("✗ Short code '{}' not found", short_code);
                return Ok(());
            }

            let url = url.unwrap();
            println!("Current created_by: {:?}", url.created_by);

            // Perform the patch
            let updated = storage.patch_created_by(&short_code, &user_id).await?;
            if updated {
                println!(
                    "✓ Updated created_by for short code '{}' to '{}'",
                    short_code, user_id
                );
            } else {
                println!(
                    "⚠ Short code '{}' was not updated (not found)",
                    short_code
                );
            }
        }
        PatchCommands::FixAll { user_id } => {
            println!(
                "⚠ This will update all malformed created_by values (NULL, empty string, or all-zero UUID)"
            );
            println!("   to user_id: '{}'", user_id);
            println!();
            println!("Checking for malformed entries...");

            // Count malformed entries before patching
            let count = storage.patch_all_malformed_created_by(&user_id).await?;

            if count > 0 {
                println!(
                    "✓ Successfully patched {} malformed created_by value(s) to '{}'",
                    count, user_id
                );
            } else {
                println!("✓ No malformed created_by values found. Database is clean!");
            }
        }
    }

    Ok(())
}

async fn handle_user_command(command: UserCommands) -> Result<()> {
    let config = Config::from_env()?;

    let storage: Arc<dyn Storage> = match config.database.backend {
        DatabaseBackend::Sqlite => Arc::new(
            SqliteStorage::new(&config.database.url, config.database.max_connections).await?,
        ),
        DatabaseBackend::Postgres => Arc::new(
            PostgresStorage::new(&config.database.url, config.database.max_connections).await?,
        ),
    };

    // Ensure database is initialized
    storage.init().await?;

    match command {
        UserCommands::List { limit, page } => {
            if page < 1 {
                println!("✗ Page number must be >= 1");
                return Ok(());
            }
            if limit < 1 {
                println!("✗ Limit must be >= 1");
                return Ok(());
            }

            let offset = (page - 1) * limit;
            let users = storage.list_all_users(limit, offset).await?;
            
            if users.is_empty() {
                if page == 1 {
                    println!("No users found.");
                } else {
                    println!("No more users found (page {}).", page);
                }
            } else {
                println!("Users (page {}, showing {} results):", page, users.len());
                println!("{:<40} {:<15} {:<40} Created At", "User ID", "Auth Method", "Email");
                println!("{}", "-".repeat(120));
                for (user_id, auth_method, email, created_at) in users {
                    let datetime = chrono::DateTime::from_timestamp(created_at, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| created_at.to_string());
                    println!("{:<40} {:<15} {:<40} {}", user_id, auth_method, email, datetime);
                }
                println!();
                println!("To see more results, use: --page {}", page + 1);
            }
        }
        UserCommands::ListAdmins => {
            let admins = storage.list_manual_admins().await?;
            if admins.is_empty() {
                println!("No manually promoted admins found.");
            } else {
                println!("Manually promoted admins:");
                println!("{:<40} {:<15} Email", "User ID", "Auth Method");
                println!("{}", "-".repeat(80));
                for (user_id, auth_method, email) in admins {
                    println!("{:<40} {:<15} {}", user_id, auth_method, email);
                }
            }
        }
        UserCommands::Links { user_id, limit, page } => {
            if page < 1 {
                println!("✗ Page number must be >= 1");
                return Ok(());
            }
            if limit < 1 {
                println!("✗ Limit must be >= 1");
                return Ok(());
            }

            let offset = (page - 1) * limit;
            let links = storage.list_user_links(&user_id, limit, offset).await?;
            
            if links.is_empty() {
                if page == 1 {
                    println!("No links found for user '{}'.", user_id);
                } else {
                    println!("No more links found for user '{}' (page {}).", user_id, page);
                }
            } else {
                println!("Links for user '{}' (page {}, showing {} results):", user_id, page, links.len());
                println!("{:<15} {:<60} {:<10} {:<10} Created At", "Short Code", "Original URL", "Clicks", "Active");
                println!("{}", "-".repeat(120));
                for link in links {
                    let url_display = if link.original_url.len() > 57 {
                        format!("{}...", &link.original_url[..57])
                    } else {
                        link.original_url.clone()
                    };
                    let active_str = if link.is_active { "✓" } else { "✗" };
                    let datetime = chrono::DateTime::from_timestamp(link.created_at, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| link.created_at.to_string());
                    println!("{:<15} {:<60} {:<10} {:<10} {}", 
                        link.short_code, url_display, link.clicks, active_str, datetime);
                }
                println!();
                println!("To see more results, use: --page {}", page + 1);
            }
        }
        UserCommands::DeactivateLinks { user_id } => {
            println!("⚠ This will mark all links created by user '{}' as inactive.", user_id);
            println!("   Note: Cached links will remain active until instance restart.");
            println!();
            
            let count = storage.bulk_deactivate_user_links(&user_id).await?;
            
            if count > 0 {
                println!("✓ Deactivated {} link(s) for user '{}'", count, user_id);
            } else {
                println!("⚠ No active links found for user '{}'", user_id);
            }
        }
        UserCommands::ReactivateLinks { user_id } => {
            println!("⚠ This will mark all links created by user '{}' as active.", user_id);
            println!("   Note: Links will become active in cache after instance restart.");
            println!();
            
            let count = storage.bulk_reactivate_user_links(&user_id).await?;
            
            if count > 0 {
                println!("✓ Reactivated {} link(s) for user '{}'", count, user_id);
            } else {
                println!("⚠ No inactive links found for user '{}'", user_id);
            }
        }
    }

    Ok(())
}

async fn run_server() -> Result<()> {
    // Load configuration
    let config = Arc::new(Config::from_env()?);
    info!("Loaded configuration");

    // Initialize cursor HMAC key
    lynx::cursor::init_cursor_hmac_key(config.pagination.cursor_hmac_secret.as_deref());
    info!("Cursor pagination HMAC key initialized");

    // Initialize storage
    let base_storage: Arc<dyn Storage> = match config.database.backend {
        DatabaseBackend::Sqlite => {
            info!("Using SQLite storage: {}", config.database.url);
            Arc::new(
                SqliteStorage::new(&config.database.url, config.database.max_connections).await?,
            )
        }
        DatabaseBackend::Postgres => {
            info!("Using PostgreSQL storage: {}", config.database.url);
            Arc::new(
                PostgresStorage::new(&config.database.url, config.database.max_connections).await?,
            )
        }
    };

    // Initialize database
    info!("Initializing database...");
    base_storage.init().await?;
    info!("Database initialized successfully");

    // Wrap with cached storage for performance
    info!(
        "Initializing cache with max {} entries, {} second DB flush interval, {} ms actor flush interval, and {} actor buffer size",
        config.cache.max_entries,
        config.cache.flush_interval_secs,
        config.cache.actor_flush_interval_ms,
        config.cache.actor_buffer_size
    );
    let cached_storage = Arc::new(CachedStorage::new(
        base_storage,
        config.cache.max_entries,
        config.cache.flush_interval_secs,
        config.cache.actor_buffer_size,
        config.cache.actor_flush_interval_ms,
    ));
    let storage: Arc<dyn Storage> = Arc::clone(&cached_storage) as Arc<dyn Storage>;

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

    // Initialize analytics if enabled
    let (analytics_config, geoip_service, analytics_aggregator) = if config.analytics.enabled {
        use lynx::analytics::{AnalyticsAggregator, GeoIpService};

        info!("📊 Analytics enabled");
        
        let city_path = config.analytics.geoip_city_db_path.as_deref();
        let asn_path = config.analytics.geoip_asn_db_path.as_deref();
        
        let geoip = match GeoIpService::new(city_path, asn_path) {
            Ok(service) => {
                if let Some(path) = city_path {
                    info!("   - GeoIP City database loaded from: {}", path);
                }
                if let Some(path) = asn_path {
                    info!("   - GeoIP ASN database loaded from: {}", path);
                }
                if city_path.is_none() && asn_path.is_none() {
                    tracing::warn!("   - No GeoIP databases configured. Analytics will have no geolocation data.");
                }
                Some(Arc::new(service))
            }
            Err(e) => {
                tracing::warn!("   - Failed to load GeoIP databases: {}. Analytics will have no geolocation data.", e);
                None
            }
        };

        let aggregator = Arc::new(AnalyticsAggregator::new());
        
        // Start optimized flush task with GeoIP service (if available)
        let storage_clone = Arc::clone(&storage);
        let _flush_handle = if let Some(ref geoip_svc) = geoip {
            // OPTIMIZED PATH: Use deferred GeoIP lookups
            let geoip_clone = Arc::clone(geoip_svc);
            aggregator.start_flush_task_with_geoip(
                config.analytics.flush_interval_secs,
                geoip_clone,
                move |entries| {
                    let storage = Arc::clone(&storage_clone);
                    Box::pin(async move {
                        if entries.is_empty() {
                            return;
                        }
                        
                        // Convert entries to storage format
                        let records: Vec<AnalyticsRecord> = entries
                            .into_iter()
                            .map(|(key, value)| {
                                (
                                    key.short_code,
                                    key.time_bucket,
                                    key.country_code,
                                    key.region,
                                    key.city,
                                    key.asn.map(|a| a as i64),
                                    key.ip_version as i32,
                                    value.count as i64,
                                )
                            })
                            .collect();
                        
                        // Batch insert to storage
                        if let Err(e) = storage.upsert_analytics_batch(records).await {
                            tracing::error!("Failed to flush analytics to storage: {}", e);
                        } else {
                            tracing::debug!("Successfully flushed analytics to storage");
                        }
                    })
                },
            )
        } else {
            // FALLBACK: No GeoIP service available, use basic flush task
            aggregator.start_flush_task_with_storage(
                config.analytics.flush_interval_secs,
                move |entries| {
                    let storage = Arc::clone(&storage_clone);
                    Box::pin(async move {
                        if entries.is_empty() {
                            return;
                        }
                        
                        // Convert entries to storage format
                        let records: Vec<AnalyticsRecord> = entries
                            .into_iter()
                            .map(|(key, value)| {
                                (
                                    key.short_code,
                                    key.time_bucket,
                                    key.country_code,
                                    key.region,
                                    key.city,
                                    key.asn.map(|a| a as i64),
                                    key.ip_version as i32,
                                    value.count as i64,
                                )
                            })
                            .collect();
                        
                        // Batch insert to storage
                        if let Err(e) = storage.upsert_analytics_batch(records).await {
                            tracing::error!("Failed to flush analytics to storage: {}", e);
                        } else {
                            tracing::debug!("Successfully flushed analytics to storage");
                        }
                    })
                },
            )
        };
        
        info!(
            "   - IP anonymization: {}",
            if config.analytics.ip_anonymization { "enabled" } else { "disabled" }
        );
        info!("   - Trusted proxy mode: {:?}", config.analytics.trusted_proxy_mode);
        info!("   - Flush interval: {} seconds", config.analytics.flush_interval_secs);

        (Some(config.analytics.clone()), geoip, Some(aggregator))
    } else {
        info!("📊 Analytics disabled");
        (None, None, None)
    };

    let api_router =
        lynx::api::create_api_router(Arc::clone(&storage), auth_service, Arc::clone(&config));
    
    // Check if timing headers should be enabled (disabled by default for max performance)
    let enable_timing_headers = std::env::var("ENABLE_TIMING_HEADERS")
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);
    
    let redirect_router = lynx::redirect::create_redirect_router(
        Arc::clone(&storage),
        analytics_config,
        geoip_service,
        analytics_aggregator,
        enable_timing_headers,
    );

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

    // Set up graceful shutdown signal
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Spawn signal handler for both SIGINT and SIGTERM
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut sigint =
                signal(SignalKind::interrupt()).expect("Failed to install SIGINT signal handler");
            let mut sigterm =
                signal(SignalKind::terminate()).expect("Failed to install SIGTERM signal handler");

            tokio::select! {
                _ = sigint.recv() => {
                    info!("Received shutdown signal (SIGINT), initiating graceful shutdown...");
                }
                _ = sigterm.recv() => {
                    info!("Received shutdown signal (SIGTERM), initiating graceful shutdown...");
                }
            }
        }

        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install CTRL+C signal handler");
            info!("Received shutdown signal (SIGINT), initiating graceful shutdown...");
        }

        let _ = shutdown_tx.send(());
    });

    // Run both servers concurrently with graceful shutdown
    let api_server = axum::serve(api_listener, api_router).with_graceful_shutdown(async {
        let _ = shutdown_rx.await;
    });

    let redirect_server = axum::serve(
        redirect_listener,
        redirect_router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    );

    // Run servers
    let result = tokio::select! {
        result = api_server => result,
        result = redirect_server => result,
    };

    // Flush cached data on shutdown
    info!("Flushing cached data before shutdown...");
    cached_storage.shutdown();
    // Wait longer to ensure both fast and slow flushes complete
    tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;
    info!("Shutdown complete");

    result?;
    Ok(())
}
