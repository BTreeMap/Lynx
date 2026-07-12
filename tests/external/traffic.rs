use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::client::ExternalService;
use super::config::StageConfig;

/// A non-empty code collection. Benchmark distributions can therefore never
/// select from an empty population at runtime.
#[derive(Clone, Debug)]
pub struct CodePool {
    first: String,
    rest: Vec<String>,
}

impl CodePool {
    pub fn new(codes: Vec<String>) -> Result<Self> {
        let mut codes = codes.into_iter();
        let first = codes.next().context("code pool must not be empty")?;
        Ok(Self {
            first,
            rest: codes.collect(),
        })
    }

    pub fn len(&self) -> usize {
        1 + self.rest.len()
    }

    fn select(&self, index: u64) -> &str {
        let index = (index % self.len() as u64) as usize;
        if index == 0 {
            &self.first
        } else {
            &self.rest[index - 1]
        }
    }
}

#[derive(Clone, Debug)]
pub enum RedirectDistribution {
    Hot {
        code: String,
    },
    Distributed {
        codes: CodePool,
    },
    Hotspot {
        hot: String,
        cold: CodePool,
        hot_percent: u8,
    },
    PowerLaw {
        head: CodePool,
        tail: CodePool,
        head_percent: u8,
    },
}

impl RedirectDistribution {
    pub fn hot(code: impl Into<String>) -> Self {
        Self::Hot { code: code.into() }
    }

    pub fn distributed(codes: Vec<String>) -> Result<Self> {
        Ok(Self::Distributed {
            codes: CodePool::new(codes)?,
        })
    }

    pub fn hotspot(hot: impl Into<String>, cold: Vec<String>, hot_percent: u8) -> Result<Self> {
        validate_percentage(hot_percent)?;
        Ok(Self::Hotspot {
            hot: hot.into(),
            cold: CodePool::new(cold)?,
            hot_percent,
        })
    }

    pub fn power_law(head: Vec<String>, tail: Vec<String>, head_percent: u8) -> Result<Self> {
        validate_percentage(head_percent)?;
        Ok(Self::PowerLaw {
            head: CodePool::new(head)?,
            tail: CodePool::new(tail)?,
            head_percent,
        })
    }

    fn select(&self, sequence: u64) -> &str {
        match self {
            Self::Hot { code } => code,
            Self::Distributed { codes } => codes.select(sequence),
            Self::Hotspot {
                hot,
                cold,
                hot_percent,
            } => {
                if sequence % 100 < *hot_percent as u64 {
                    hot
                } else {
                    cold.select(sequence)
                }
            }
            Self::PowerLaw {
                head,
                tail,
                head_percent,
            } => {
                if sequence % 100 < *head_percent as u64 {
                    head.select(sequence)
                } else {
                    tail.select(sequence)
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum TrafficPlan {
    Redirect(RedirectDistribution),
    GetUrl {
        code: String,
    },
    ListUrls,
    Health,
    CreateUrl,
    Deactivate {
        codes: CodePool,
    },
    Analytics {
        code: String,
    },
    /// 80% redirects, 15% management reads, 5% management writes.
    Mixed {
        redirects: RedirectDistribution,
    },
}

impl TrafficPlan {
    pub fn description(&self) -> &'static str {
        match self {
            Self::Redirect(RedirectDistribution::Hot { .. }) => "redirect-hot",
            Self::Redirect(RedirectDistribution::Distributed { .. }) => "redirect-distributed",
            Self::Redirect(RedirectDistribution::Hotspot { .. }) => "redirect-hotspot",
            Self::Redirect(RedirectDistribution::PowerLaw { .. }) => "redirect-power-law",
            Self::GetUrl { .. } => "api-get-url",
            Self::ListUrls => "api-list-urls",
            Self::Health => "api-health",
            Self::CreateUrl => "api-create-url",
            Self::Deactivate { .. } => "api-deactivate-url",
            Self::Analytics { .. } => "api-analytics",
            Self::Mixed { .. } => "mixed-80-15-5",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSnapshot {
    pub total: u64,
    pub success: u64,
    pub unexpected_status: u64,
    pub client_error: u64,
    pub server_error: u64,
    pub transport_error: u64,
    pub error_rate: f64,
    pub requests_per_second: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub latency_samples: usize,
}

struct AtomicStats {
    total: AtomicU64,
    success: AtomicU64,
    unexpected_status: AtomicU64,
    client_error: AtomicU64,
    server_error: AtomicU64,
    transport_error: AtomicU64,
    sample_clock: AtomicU64,
    sample_stride: u64,
    latency_micros: Mutex<Vec<u32>>,
}

impl AtomicStats {
    fn new(sample_stride: NonZeroUsize, sample_capacity_hint: usize) -> Self {
        Self {
            total: AtomicU64::new(0),
            success: AtomicU64::new(0),
            unexpected_status: AtomicU64::new(0),
            client_error: AtomicU64::new(0),
            server_error: AtomicU64::new(0),
            transport_error: AtomicU64::new(0),
            sample_clock: AtomicU64::new(0),
            sample_stride: sample_stride.get() as u64,
            latency_micros: Mutex::new(Vec::with_capacity(sample_capacity_hint)),
        }
    }

    fn record_status(&self, status: reqwest::StatusCode, expected: bool, latency: Duration) {
        self.total.fetch_add(1, Ordering::Relaxed);
        if expected {
            self.success.fetch_add(1, Ordering::Relaxed);
        } else if status.is_client_error() {
            self.client_error.fetch_add(1, Ordering::Relaxed);
        } else if status.is_server_error() {
            self.server_error.fetch_add(1, Ordering::Relaxed);
        } else {
            self.unexpected_status.fetch_add(1, Ordering::Relaxed);
        }
        self.record_latency(latency);
    }

    fn record_transport_error(&self, latency: Duration) {
        self.total.fetch_add(1, Ordering::Relaxed);
        self.transport_error.fetch_add(1, Ordering::Relaxed);
        self.record_latency(latency);
    }

    fn record_latency(&self, latency: Duration) {
        let sample = self.sample_clock.fetch_add(1, Ordering::Relaxed);
        if !sample.is_multiple_of(self.sample_stride) {
            return;
        }

        let micros = u32::try_from(latency.as_micros()).unwrap_or(u32::MAX);
        self.latency_micros
            .lock()
            .expect("latency collector mutex must not be poisoned")
            .push(micros);
    }

    fn snapshot(&self, elapsed: Duration) -> TrafficSnapshot {
        let mut samples = self
            .latency_micros
            .lock()
            .expect("latency collector mutex must not be poisoned")
            .clone();
        samples.sort_unstable();

        let percentile = |quantile: f64| -> f64 {
            if samples.is_empty() {
                return 0.0;
            }
            let index = ((samples.len() - 1) as f64 * quantile).round() as usize;
            samples[index] as f64 / 1_000.0
        };

        let total = self.total.load(Ordering::Relaxed);
        let success = self.success.load(Ordering::Relaxed);
        let unexpected_status = self.unexpected_status.load(Ordering::Relaxed);
        let client_error = self.client_error.load(Ordering::Relaxed);
        let server_error = self.server_error.load(Ordering::Relaxed);
        let transport_error = self.transport_error.load(Ordering::Relaxed);

        TrafficSnapshot {
            total,
            success,
            unexpected_status,
            client_error,
            server_error,
            transport_error,
            error_rate: if total == 0 {
                0.0
            } else {
                total.saturating_sub(success) as f64 / total as f64
            },
            requests_per_second: if elapsed.is_zero() {
                0.0
            } else {
                total as f64 / elapsed.as_secs_f64()
            },
            p50_ms: percentile(0.50),
            p95_ms: percentile(0.95),
            p99_ms: percentile(0.99),
            latency_samples: samples.len(),
        }
    }
}

/// Run a deadline-bound, native Rust traffic stage.
///
/// Workers own no shared mutable request state. They receive a cheap clone of
/// the reqwest client, publish only lock-free counters on the hot path, and
/// self-terminate at the deadline. A bounded join acts as a liveness backstop
/// for stalled sockets or an overloaded runner.
pub async fn run_stage(
    service: ExternalService,
    plan: TrafficPlan,
    config: StageConfig,
) -> Result<TrafficSnapshot> {
    let stats = Arc::new(AtomicStats::new(
        config.latency_sample_stride,
        config.concurrency.get().saturating_mul(4),
    ));
    let sequence = Arc::new(AtomicU64::new(0));
    let started = Instant::now();
    let deadline = started + config.duration;
    let mut workers = Vec::with_capacity(config.concurrency.get());

    for _ in 0..config.concurrency.get() {
        let service = service.clone();
        let plan = plan.clone();
        let stats = Arc::clone(&stats);
        let sequence = Arc::clone(&sequence);
        workers.push(tokio::spawn(async move {
            while Instant::now() < deadline {
                let request_sequence = sequence.fetch_add(1, Ordering::Relaxed);
                let request_started = Instant::now();
                match execute_request(&service, &plan, request_sequence).await {
                    Ok((status, expected)) => {
                        stats.record_status(status, expected, request_started.elapsed());
                    }
                    Err(_) => {
                        stats.record_transport_error(request_started.elapsed());
                        // Fast connection failures can otherwise monopolize a
                        // worker at high concurrency and starve other tasks.
                        tokio::task::yield_now().await;
                    }
                }
            }
        }));
    }

    let join_budget = config.duration + Duration::from_secs(20);
    let joined = tokio::time::timeout(join_budget, async {
        for worker in &mut workers {
            worker.await.context("native traffic worker panicked")?;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;

    match joined {
        Ok(result) => result?,
        Err(_) => {
            for worker in workers {
                worker.abort();
            }
            bail!(
                "native traffic stage exceeded its {:?} completion budget",
                join_budget
            );
        }
    }

    Ok(stats.snapshot(started.elapsed()))
}

async fn execute_request(
    service: &ExternalService,
    plan: &TrafficPlan,
    sequence: u64,
) -> Result<(reqwest::StatusCode, bool)> {
    let (status, expected) = match plan {
        TrafficPlan::Redirect(distribution) => {
            let status = service
                .redirect_status(distribution.select(sequence))
                .await?;
            (status, status.is_redirection())
        }
        TrafficPlan::GetUrl { code } => {
            let status = service.url_status_for_traffic(code).await?;
            (status, status.is_success())
        }
        TrafficPlan::ListUrls => {
            let status = service.list_status_for_traffic().await?;
            (status, status.is_success())
        }
        TrafficPlan::Health => {
            let status = service.health_status_for_traffic().await?;
            (status, status.is_success())
        }
        TrafficPlan::CreateUrl => {
            let status = service.create_status_for_traffic(sequence).await?;
            (status, status == reqwest::StatusCode::CREATED)
        }
        TrafficPlan::Deactivate { codes } => {
            let status = service
                .deactivate_status_for_traffic(codes.select(sequence))
                .await?;
            (status, status.is_success())
        }
        TrafficPlan::Analytics { code } => {
            let status = service.analytics_status_for_traffic(code).await?;
            (status, status.is_success())
        }
        TrafficPlan::Mixed { redirects } => match sequence % 20 {
            0 => {
                let status = service.create_status_for_traffic(sequence).await?;
                (status, status == reqwest::StatusCode::CREATED)
            }
            1..=3 => {
                let status = service
                    .url_status_for_traffic(redirects.select(sequence))
                    .await?;
                (status, status.is_success())
            }
            _ => {
                let status = service.redirect_status(redirects.select(sequence)).await?;
                (status, status.is_redirection())
            }
        },
    };
    Ok((status, expected))
}

fn validate_percentage(value: u8) -> Result<()> {
    if value > 100 {
        bail!("traffic distribution percentage must not exceed 100");
    }
    Ok(())
}
