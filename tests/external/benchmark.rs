use anyhow::{bail, ensure, Context, Result};
use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::task::JoinSet;

use super::client::ExternalService;
use super::config::{duration_from_env, usize_from_env, ExternalConfig, StageConfig};
use super::report::{new_report, write_report, BenchmarkReport, BenchmarkRun};
use super::scenarios::CodeFactory;
use super::traffic::{run_stage, RedirectDistribution, TrafficPlan};

#[derive(Clone, Copy, Debug)]
enum BenchmarkSuite {
    Standard,
    Analytics,
}

impl BenchmarkSuite {
    fn from_env() -> Result<Self> {
        match env::var("BENCHMARK_SUITE")
            .unwrap_or_else(|_| "standard".to_owned())
            .as_str()
        {
            "standard" => Ok(Self::Standard),
            "analytics" => Ok(Self::Analytics),
            other => bail!("BENCHMARK_SUITE must be `standard` or `analytics`, got {other:?}"),
        }
    }
}

struct BenchmarkConfig {
    external: ExternalConfig,
    suite: BenchmarkSuite,
    duration: Duration,
    max_concurrency: usize,
    output_dir: PathBuf,
    label: String,
    baseline_report: Option<PathBuf>,
}

impl BenchmarkConfig {
    fn from_env() -> Result<Self> {
        let output_dir = env::var("BENCHMARK_OUTPUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("benchmark-results"));
        let label = env::var("BENCHMARK_LABEL").unwrap_or_else(|_| "default".to_owned());
        ensure!(
            !label.is_empty()
                && label
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-'),
            "BENCHMARK_LABEL must be non-empty ASCII alphanumeric text with optional hyphens"
        );

        Ok(Self {
            external: ExternalConfig::from_env()?,
            suite: BenchmarkSuite::from_env()?,
            duration: duration_from_env("BENCHMARK_DURATION_SECS", 30)?,
            max_concurrency: usize_from_env("BENCHMARK_MAX_CONCURRENCY", 10_000)?,
            output_dir,
            label,
            baseline_report: env::var("BENCHMARK_COMPARE_BASELINE")
                .ok()
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from),
        })
    }

    fn stage(&self, concurrency: usize, duration: Duration) -> Result<StageConfig> {
        StageConfig::new(concurrency.min(self.max_concurrency), duration)
    }

    fn redirect_concurrencies(&self) -> Vec<usize> {
        let mut values = BTreeSet::new();
        for candidate in [1_000, 5_000, 10_000] {
            values.insert(candidate.min(self.max_concurrency));
        }
        values.into_iter().collect()
    }
}

pub async fn run_from_env() -> Result<PathBuf> {
    let config = BenchmarkConfig::from_env()?;
    let service = ExternalService::new(&config.external)?;
    service
        .wait_until_ready(config.external.readiness_timeout)
        .await?;

    let auth = service.auth_mode().await?;
    ensure!(
        auth.mode == "none",
        "benchmark harness requires AUTH_MODE=none"
    );
    let codes = CodeFactory::new(auth.short_code_max_length)?;
    let mut report = new_report(
        config.label.clone(),
        service.api_base().to_owned(),
        service.redirect_base().to_owned(),
    )?;

    match config.suite {
        BenchmarkSuite::Standard => run_standard(&service, &codes, &config, &mut report).await?,
        BenchmarkSuite::Analytics => run_analytics(&service, &codes, &config, &mut report).await?,
    }

    let output = write_report(
        &config.output_dir,
        &report,
        config.baseline_report.as_deref(),
    )?;
    println!("native benchmark report: {}", output.display());
    Ok(output)
}

async fn run_standard(
    service: &ExternalService,
    codes: &CodeFactory,
    config: &BenchmarkConfig,
    report: &mut BenchmarkReport,
) -> Result<()> {
    let benchmark_codes = setup_urls(service, codes, 100, "standard").await?;
    let hot_code = benchmark_codes
        .first()
        .context("standard benchmark setup generated no URLs")?
        .clone();
    let distributed = RedirectDistribution::distributed(benchmark_codes.clone())?;

    for concurrency in config.redirect_concurrencies() {
        record_stage(
            report,
            &format!("redirect-hot-{concurrency}"),
            TrafficPlan::Redirect(RedirectDistribution::hot(hot_code.clone())),
            config.stage(concurrency, config.duration)?,
            service,
        )
        .await?;
    }
    record_stage(
        report,
        "redirect-distributed-1000",
        TrafficPlan::Redirect(distributed.clone()),
        config.stage(1_000, config.duration)?,
        service,
    )
    .await?;

    record_stage(
        report,
        "api-create-100",
        TrafficPlan::CreateUrl,
        config.stage(100, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "api-get-500",
        TrafficPlan::GetUrl {
            code: hot_code.clone(),
        },
        config.stage(500, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "api-list-100",
        TrafficPlan::ListUrls,
        config.stage(100, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "api-health-1000",
        TrafficPlan::Health,
        config.stage(1_000, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "mixed-80-15-5-1000",
        TrafficPlan::Mixed {
            redirects: distributed,
        },
        config.stage(1_000, config.duration)?,
        service,
    )
    .await?;

    record_stage(
        report,
        "api-deactivate-100",
        TrafficPlan::Deactivate {
            codes: super::traffic::CodePool::new(benchmark_codes.clone())?,
        },
        config.stage(100, config.duration)?,
        service,
    )
    .await?;

    for code in &benchmark_codes {
        service.reactivate(code).await?;
    }

    Ok(())
}

async fn run_analytics(
    service: &ExternalService,
    codes: &CodeFactory,
    config: &BenchmarkConfig,
    report: &mut BenchmarkReport,
) -> Result<()> {
    let benchmark_codes = setup_urls(service, codes, 500, "analytics").await?;
    let hot_code = benchmark_codes
        .first()
        .context("analytics benchmark setup generated no URLs")?
        .clone();
    let first_hundred = benchmark_codes
        .iter()
        .take(100)
        .cloned()
        .collect::<Vec<_>>();
    let top_ten = benchmark_codes.iter().take(10).cloned().collect::<Vec<_>>();
    let tail_ninety = benchmark_codes
        .iter()
        .skip(10)
        .take(90)
        .cloned()
        .collect::<Vec<_>>();

    for concurrency in config.redirect_concurrencies() {
        record_stage(
            report,
            &format!("analytics-hot-{concurrency}"),
            TrafficPlan::Redirect(RedirectDistribution::hot(hot_code.clone())),
            config.stage(concurrency, config.duration)?,
            service,
        )
        .await?;
    }
    record_stage(
        report,
        "analytics-distributed-100-1000",
        TrafficPlan::Redirect(RedirectDistribution::distributed(first_hundred.clone())?),
        config.stage(1_000, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "analytics-distributed-500-1000",
        TrafficPlan::Redirect(RedirectDistribution::distributed(benchmark_codes.clone())?),
        config.stage(1_000, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "analytics-distributed-100-5000",
        TrafficPlan::Redirect(RedirectDistribution::distributed(first_hundred.clone())?),
        config.stage(5_000, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "analytics-hotspot-1000",
        TrafficPlan::Redirect(RedirectDistribution::hotspot(
            hot_code.clone(),
            first_hundred.iter().skip(1).cloned().collect(),
            80,
        )?),
        config.stage(1_000, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "analytics-power-law-1000",
        TrafficPlan::Redirect(RedirectDistribution::power_law(top_ten, tail_ninety, 70)?),
        config.stage(1_000, config.duration)?,
        service,
    )
    .await?;
    record_stage(
        report,
        "analytics-sustained-1000",
        TrafficPlan::Redirect(RedirectDistribution::distributed(first_hundred)?),
        config.stage(1_000, config.duration.saturating_mul(2))?,
        service,
    )
    .await?;
    record_stage(
        report,
        "analytics-api-100",
        TrafficPlan::Analytics { code: hot_code },
        config.stage(100, config.duration)?,
        service,
    )
    .await?;

    Ok(())
}

async fn record_stage(
    report: &mut BenchmarkReport,
    name: &str,
    plan: TrafficPlan,
    stage: StageConfig,
    service: &ExternalService,
) -> Result<()> {
    println!(
        "native benchmark stage {name}: {} workers for {:?}",
        stage.concurrency, stage.duration
    );
    let workload = plan.description().to_owned();
    let snapshot = run_stage(service.clone(), plan, stage).await?;
    println!(
        "native benchmark stage {name}: {:.2} RPS, {:.2}% errors, p99 {:.3} ms",
        snapshot.requests_per_second,
        snapshot.error_rate * 100.0,
        snapshot.p99_ms
    );
    report.runs.push(BenchmarkRun {
        name: name.to_owned(),
        workload,
        concurrency: stage.concurrency.get(),
        duration_seconds: stage.duration.as_secs(),
        snapshot,
    });
    Ok(())
}

async fn setup_urls(
    service: &ExternalService,
    codes: &CodeFactory,
    count: usize,
    category: &str,
) -> Result<Vec<String>> {
    const SETUP_CONCURRENCY: usize = 32;
    let mut tasks = JoinSet::new();
    let mut created = Vec::with_capacity(count);

    for index in 0..count {
        while tasks.len() >= SETUP_CONCURRENCY {
            let code = tasks
                .join_next()
                .await
                .context("benchmark setup task set unexpectedly empty")?
                .context("benchmark setup task panicked")??;
            created.push(code);
        }

        let service = service.clone();
        let code = codes.code();
        let target = format!("https://example.com/benchmark/{category}/{index}");
        tasks.spawn(async move {
            let created = service.create(&target, Some(&code)).await?;
            ensure!(
                created.short_code == code,
                "benchmark setup returned wrong code"
            );
            Ok::<String, anyhow::Error>(code)
        });
    }

    while let Some(result) = tasks.join_next().await {
        created.push(result.context("benchmark setup task panicked")??);
    }
    created.sort_unstable();
    ensure!(
        created.len() == count,
        "benchmark setup did not create {count} URLs"
    );
    Ok(created)
}
