use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lynx::config::{Config, DatabaseBackend};
use lynx::storage::{PostgresStorage, SqliteStorage, Storage};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "lynx-admin")]
#[command(about = "Lynx admin management CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Promote a user to admin
    Promote {
        /// User ID (sub claim from JWT)
        user_id: String,
        /// Authentication method (oauth, cloudflare, none)
        auth_method: String,
    },
    /// Demote a user from admin
    Demote {
        /// User ID (sub claim from JWT)
        user_id: String,
        /// Authentication method (oauth, cloudflare, none)
        auth_method: String,
    },
    /// List all manually promoted admins
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = Config::from_env()?;

    let storage: Arc<dyn Storage> = match config.database.backend {
        DatabaseBackend::Sqlite => {
            Arc::new(SqliteStorage::new(&config.database.url).await?)
        }
        DatabaseBackend::Postgres => {
            Arc::new(PostgresStorage::new(&config.database.url).await?)
        }
    };

    // Ensure database is initialized
    storage.init().await?;

    match cli.command {
        Commands::Promote {
            user_id,
            auth_method,
        } => {
            storage.promote_to_admin(&user_id, &auth_method).await?;
            println!(
                "✓ Promoted user '{}' with auth method '{}' to admin",
                user_id, auth_method
            );
        }
        Commands::Demote {
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
        Commands::List => {
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
