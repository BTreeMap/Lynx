//! Rust-native performance and CPU-profiling harness.
//!
//! The profiling test is ignored by default and compiled only with the
//! `profiling` feature:
//! `cargo test --profile profiling --features profiling --test performance_harness -- --ignored --nocapture`
#![cfg(feature = "profiling")]

#[path = "performance_harness/profiling.rs"]
mod profiling;

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use axum::http::StatusCode;
use axum::Router;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use lynx::api::create_api_router;
use lynx::auth::AuthService;
use lynx::config::{
    AnalyticsConfig, AuthConfig, AuthMode, CacheConfig, Config, DatabaseBackend, DatabaseConfig,
    FrontendConfig, PaginationConfig, RedirectMode, ServerConfig,
};
use lynx::redirect::create_redirect_router;
use lynx::storage::{CachedStorage, PostgresStorage, Storage};
use reqwest::redirect::Policy;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};

use profiling::{FlamegraphConfig, ProfileMetric, ProfileScenario, ProfileSession};

const SEED_URL_COUNT: usize = 20;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

struct Harness {
    api_base: String,
    redirect_base: String,
    client: reqwest::Client,
    cached_storage: Arc<CachedStorage>,
    servers: [JoinHandle<()>; 2],
}

impl Harness {
    async fn start() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL is required by the PostgreSQL performance harness")?;
        let base_storage: Arc<dyn Storage> = Arc::new(
            PostgresStorage::new(&database_url, 50)
                .await
                .context("connect performance harness to PostgreSQL")?,
        );
        base_storage
            .init()
            .await
            .context("initialize performance harness schema")?;

        let cached_storage = Arc::new(CachedStorage::new(base_storage, 500_000, 5, 1_000_000, 100));
        let storage: Arc<dyn Storage> = Arc::clone(&cached_storage) as Arc<dyn Storage>;
        let config = Arc::new(harness_config(database_url));
        let auth = Arc::new(
            AuthService::new(config.auth.clone())
                .await
                .context("create performance harness auth service")?,
        );

        let api = create_api_router(Arc::clone(&storage), auth, config, None);
        let redirect = create_redirect_router(
            Arc::clone(&cached_storage),
            None,
            false,
            StatusCode::PERMANENT_REDIRECT,
        );
        let (api_base, api_server) = serve(api).await?;
        let (redirect_base, redirect_server) = serve_with_connect_info(redirect).await?;
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(Policy::none())
            .build()
            .context("build performance harness HTTP client")?;

        Ok(Self {
            api_base,
            redirect_base,
            client,
            cached_storage,
            servers: [api_server, redirect_server],
        })
    }

    async fn seed(&self, scenario: ProfileScenario) -> Result<()> {
        for index in 1..=SEED_URL_COUNT {
            let response = self
                .client
                .post(format!("{}/api/urls", self.api_base))
                .json(&json!({
                    "url": format!("https://example.com/profile-target-{index}"),
                    "custom_code": scenario.code(index),
                }))
                .send()
                .await
                .with_context(|| format!("seed {} URL {index}", scenario.label()))?;
            ensure!(
                response.status().is_success(),
                "seed {} URL {index} failed with {}",
                scenario.label(),
                response.status()
            );
        }
        Ok(())
    }

    async fn warm_redirect(&self, scenario: ProfileScenario) -> Result<()> {
        let url = format!("{}/{}", self.redirect_base, scenario.code(1));
        for _ in 0..100 {
            let response = self.client.get(&url).send().await?;
            let status = response.status();
            ensure!(
                status.is_redirection(),
                "warm-up did not redirect: {status} from {url}"
            );
        }
        Ok(())
    }

    async fn verify_api_reads(&self, scenario: ProfileScenario) -> Result<()> {
        for index in 1..=SEED_URL_COUNT {
            let code = scenario.code(index);
            let url = api_url(&self.api_base, &code);
            let response = self.client.get(&url).send().await?;
            let status = response.status();
            ensure!(
                status.is_success(),
                "seeded API read failed: {status} from {url}"
            );
        }
        Ok(())
    }

    async fn run(&self, scenario: ProfileScenario, config: &FlamegraphConfig) -> Result<Snapshot> {
        let deadline = Instant::now() + config.duration();
        let total = Arc::new(AtomicU64::new(0));
        let errors = Arc::new(AtomicU64::new(0));
        let mut workers = JoinSet::new();

        for worker in 0..config.concurrency(scenario).get() {
            let client = self.client.clone();
            let api_base = self.api_base.clone();
            let redirect_base = self.redirect_base.clone();
            let total = Arc::clone(&total);
            let errors = Arc::clone(&errors);
            workers.spawn(async move {
                let mut sequence = worker as u64;
                while Instant::now() < deadline {
                    let result = send_request(
                        &client,
                        scenario,
                        &api_base,
                        &redirect_base,
                        worker,
                        sequence,
                    )
                    .await;
                    total.fetch_add(1, Ordering::Relaxed);
                    if !matches!(result, Ok(status) if scenario.accepts(status)) {
                        errors.fetch_add(1, Ordering::Relaxed);
                        tokio::task::yield_now().await;
                    }
                    sequence = sequence.wrapping_add(1);
                }
            });
        }

        while let Some(result) = workers.join_next().await {
            result.context("performance worker panicked")?;
        }

        let total = total.load(Ordering::Relaxed);
        let errors = errors.load(Ordering::Relaxed);
        Ok(Snapshot {
            total,
            errors,
            requests_per_second: total as f64 / config.duration().as_secs_f64(),
        })
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for server in &self.servers {
            server.abort();
        }
    }
}

#[derive(Debug)]
struct Snapshot {
    total: u64,
    errors: u64,
    requests_per_second: f64,
}

async fn send_request(
    client: &reqwest::Client,
    scenario: ProfileScenario,
    api_base: &str,
    redirect_base: &str,
    worker: usize,
    sequence: u64,
) -> reqwest::Result<StatusCode> {
    let response = match scenario {
        ProfileScenario::RedirectCached => {
            client
                .get(format!("{redirect_base}/{}", scenario.code(1)))
                .send()
                .await?
        }
        ProfileScenario::ApiMixed if sequence.is_multiple_of(2) => {
            client
                .post(format!("{api_base}/api/urls"))
                .json(&json!({
                    "url": format!("https://example.com/profile-api-{worker}-{sequence}"),
                }))
                .send()
                .await?
        }
        ProfileScenario::ApiMixed => {
            let index = sequence as usize % SEED_URL_COUNT + 1;
            client
                .get(api_url(api_base, &scenario.code(index)))
                .send()
                .await?
        }
    };
    Ok(response.status())
}

fn api_url(api_base: &str, code: &str) -> String {
    format!("{api_base}/api/urls/{}", URL_SAFE_NO_PAD.encode(code))
}

async fn serve(router: Router) -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("bind performance harness server")?;
    let address = listener.local_addr().context("read harness address")?;
    let server = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            panic!("performance harness server failed: {error}");
        }
    });
    Ok((format!("http://{address}"), server))
}

async fn serve_with_connect_info(router: Router) -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("bind performance harness server")?;
    let address = listener.local_addr().context("read harness address")?;
    let server = tokio::spawn(async move {
        if let Err(error) = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        {
            panic!("performance harness server failed: {error}");
        }
    });
    Ok((format!("http://{address}"), server))
}

fn harness_config(database_url: String) -> Config {
    Config {
        database: DatabaseConfig {
            backend: DatabaseBackend::Postgres,
            url: database_url,
            max_connections: 50,
        },
        api_server: ServerConfig {
            host: "127.0.0.1".into(),
            port: 0,
        },
        redirect_server: ServerConfig {
            host: "127.0.0.1".into(),
            port: 0,
        },
        redirect_base_url: "http://127.0.0.1".into(),
        auth: AuthConfig {
            mode: AuthMode::None,
            oauth: None,
            cloudflare: None,
        },
        frontend: FrontendConfig { static_dir: None },
        cache: CacheConfig {
            max_entries: 500_000,
            flush_interval_secs: 5,
            actor_buffer_size: 1_000_000,
            actor_flush_interval_ms: 100,
        },
        pagination: PaginationConfig {
            cursor_hmac_secret: None,
        },
        short_code_max_length: 50,
        analytics: AnalyticsConfig::default(),
        redirect_status: RedirectMode::Permanent,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 12)]
#[ignore = "CPU profiling harness; run in the dedicated performance workflow"]
async fn representative_hot_path_flamegraphs() -> Result<()> {
    let config = FlamegraphConfig::from_env()?;
    let harness = Harness::start().await?;
    let mut metrics = Vec::new();

    for &scenario in config.scenarios() {
        harness.seed(scenario).await?;
        match scenario {
            ProfileScenario::RedirectCached => harness.warm_redirect(scenario).await?,
            ProfileScenario::ApiMixed => harness.verify_api_reads(scenario).await?,
        }

        eprintln!(
            "[profile] {}: {}s at c{} and {} Hz",
            scenario.label(),
            config.duration().as_secs(),
            config.concurrency(scenario),
            config.frequency_hz()
        );
        let profile = ProfileSession::start(&config, scenario)?;
        let snapshot = harness.run(scenario, &config).await?;
        profile.finish()?;

        ensure!(
            snapshot.total > 0,
            "{} completed no requests",
            scenario.label()
        );
        ensure!(
            snapshot.errors == 0,
            "{} completed with {} errors",
            scenario.label(),
            snapshot.errors
        );
        eprintln!(
            "[profile] {}: {} requests ({:.1} req/s)",
            scenario.label(),
            snapshot.total,
            snapshot.requests_per_second
        );
        metrics.push(ProfileMetric::new(&config, scenario, &snapshot));
    }

    config.write_guide()?;
    config.write_metrics(&metrics)?;
    harness.cached_storage.shutdown().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ConnectInfo;
    use axum::routing::get;
    use std::net::SocketAddr;

    #[test]
    fn scenario_concurrency_is_nonzero() {
        let config = FlamegraphConfig::default();
        for &scenario in config.scenarios() {
            assert!(config.concurrency(scenario) >= NonZeroUsize::MIN);
        }
    }

    #[test]
    fn management_api_url_encodes_short_code() {
        assert_eq!(api_url("http://api", "a/b"), "http://api/api/urls/YS9i");
    }

    #[tokio::test]
    async fn redirect_server_injects_peer_address() {
        async fn peer_address(ConnectInfo(address): ConnectInfo<SocketAddr>) -> String {
            address.ip().to_string()
        }

        let router = Router::new().route("/", get(peer_address));
        let (base, server) = serve_with_connect_info(router).await.unwrap();
        let response = reqwest::get(base).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), "127.0.0.1");
        server.abort();
    }
}
