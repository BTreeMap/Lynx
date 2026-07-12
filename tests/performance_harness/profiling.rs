use std::fs::File;
use std::num::{NonZeroU64, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use super::Snapshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProfileScenario {
    RedirectCached,
    ApiMixed,
}

impl ProfileScenario {
    pub(super) const ALL: [Self; 2] = [Self::RedirectCached, Self::ApiMixed];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::RedirectCached => "redirect-cached",
            Self::ApiMixed => "api-mixed",
        }
    }

    const fn file_name(self) -> &'static str {
        match self {
            Self::RedirectCached => "flamegraph-redirect-cached.svg",
            Self::ApiMixed => "flamegraph-api-operations.svg",
        }
    }

    pub(super) fn code(self, index: usize) -> String {
        format!("prof-{}-{index}", self.label())
    }

    pub(super) fn accepts(self, status: reqwest::StatusCode) -> bool {
        match self {
            Self::RedirectCached => status.is_redirection(),
            Self::ApiMixed => status.is_success(),
        }
    }

    fn parse(raw: &str) -> Result<Self> {
        match raw {
            "redirect-cached" => Ok(Self::RedirectCached),
            "api-mixed" => Ok(Self::ApiMixed),
            _ => bail!(
                "PERF_FLAMEGRAPH_SCENARIOS contains unknown scenario {raw:?}; expected redirect-cached or api-mixed"
            ),
        }
    }
}

#[derive(Debug)]
pub(super) struct FlamegraphConfig {
    output_dir: PathBuf,
    frequency_hz: i32,
    duration: Duration,
    redirect_concurrency: NonZeroUsize,
    api_concurrency: NonZeroUsize,
    scenarios: Vec<ProfileScenario>,
}

impl Default for FlamegraphConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("target/flamegraphs"),
            frequency_hz: 99,
            duration: Duration::from_secs(15),
            redirect_concurrency: NonZeroUsize::new(256).expect("constant is non-zero"),
            api_concurrency: NonZeroUsize::new(64).expect("constant is non-zero"),
            scenarios: ProfileScenario::ALL.to_vec(),
        }
    }
}

impl FlamegraphConfig {
    pub(super) fn from_env() -> Result<Self> {
        let default = Self::default();
        let frequency =
            env_nonzero_u64("PERF_FLAMEGRAPH_FREQUENCY_HZ", default.frequency_hz as u64)?;
        let frequency_hz = i32::try_from(frequency.get())
            .context("PERF_FLAMEGRAPH_FREQUENCY_HZ must fit in i32")?;

        Ok(Self {
            output_dir: std::env::var_os("PERF_FLAMEGRAPH_OUTPUT_DIR")
                .map(PathBuf::from)
                .unwrap_or(default.output_dir),
            frequency_hz,
            duration: env_duration("PERF_FLAMEGRAPH_DURATION", default.duration)?,
            redirect_concurrency: env_nonzero_usize(
                "PERF_FLAMEGRAPH_REDIRECT_CONCURRENCY",
                default.redirect_concurrency.get(),
            )?,
            api_concurrency: env_nonzero_usize(
                "PERF_FLAMEGRAPH_API_CONCURRENCY",
                default.api_concurrency.get(),
            )?,
            scenarios: env_scenarios("PERF_FLAMEGRAPH_SCENARIOS")?,
        })
    }

    pub(super) const fn frequency_hz(&self) -> i32 {
        self.frequency_hz
    }

    pub(super) const fn duration(&self) -> Duration {
        self.duration
    }

    pub(super) const fn concurrency(&self, scenario: ProfileScenario) -> NonZeroUsize {
        match scenario {
            ProfileScenario::RedirectCached => self.redirect_concurrency,
            ProfileScenario::ApiMixed => self.api_concurrency,
        }
    }

    pub(super) fn scenarios(&self) -> &[ProfileScenario] {
        &self.scenarios
    }

    fn output_path(&self, scenario: ProfileScenario) -> PathBuf {
        self.output_dir.join(scenario.file_name())
    }

    pub(super) fn write_guide(&self) -> Result<()> {
        std::fs::create_dir_all(&self.output_dir).with_context(|| {
            format!("create flamegraph directory {}", self.output_dir.display())
        })?;
        let guide = format!(
            "# Lynx Hot-Path Flamegraphs\n\n\
             These interactive SVGs were sampled in-process from a release-optimized Lynx\n\
             build with symbols and frame pointers. The normal release build is unchanged.\n\n\
             | Scenario | Representative workload | Sampling |\n\
             |---|---|---|\n\
             | Cached redirect | {} concurrent readers of one warmed short code | {}s at {} Hz |\n\
             | Mixed API | {} concurrent workers alternating creates and reads | {}s at {} Hz |\n\n\
             Open an SVG in a browser, click frames to zoom, and search for `lynx::redirect`,\n\
             `lynx::storage`, `moka`, `sqlx`, allocation, locking, and Tokio scheduler frames.\n",
            self.redirect_concurrency,
            self.duration.as_secs(),
            self.frequency_hz,
            self.api_concurrency,
            self.duration.as_secs(),
            self.frequency_hz,
        );
        std::fs::write(self.output_dir.join("README.md"), guide)
            .context("write flamegraph interpretation guide")
    }

    pub(super) fn write_metrics(&self, metrics: &[ProfileMetric]) -> Result<()> {
        std::fs::create_dir_all(&self.output_dir).with_context(|| {
            format!("create flamegraph directory {}", self.output_dir.display())
        })?;
        let report = ProfileMetrics {
            schema_version: 1,
            commit: std::env::var("GITHUB_SHA").ok(),
            runner_os: std::env::var("RUNNER_OS").ok(),
            runner_arch: std::env::var("RUNNER_ARCH").ok(),
            scenarios: metrics,
        };
        let output = serde_json::to_vec_pretty(&report).context("serialize profile metrics")?;
        std::fs::write(self.output_dir.join("metrics.json"), output)
            .context("write profile metrics")
    }
}

fn env_scenarios(key: &str) -> Result<Vec<ProfileScenario>> {
    let raw = match std::env::var(key) {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => return Ok(ProfileScenario::ALL.to_vec()),
        Err(std::env::VarError::NotUnicode(_)) => bail!("{key} must contain valid UTF-8"),
    };
    parse_scenarios(key, &raw)
}

fn parse_scenarios(key: &str, raw: &str) -> Result<Vec<ProfileScenario>> {
    if raw == "all" {
        return Ok(ProfileScenario::ALL.to_vec());
    }

    let mut scenarios = Vec::new();
    for raw_scenario in raw.split(',').map(str::trim) {
        let scenario = ProfileScenario::parse(raw_scenario)?;
        if scenarios.contains(&scenario) {
            bail!("{key} must not contain duplicate scenarios");
        }
        scenarios.push(scenario);
    }
    if scenarios.is_empty() {
        bail!("{key} must select at least one scenario");
    }
    Ok(scenarios)
}

#[derive(Serialize)]
struct ProfileMetrics<'a> {
    schema_version: u8,
    commit: Option<String>,
    runner_os: Option<String>,
    runner_arch: Option<String>,
    scenarios: &'a [ProfileMetric],
}

#[derive(Debug, Serialize)]
pub(super) struct ProfileMetric {
    scenario: &'static str,
    requests: u64,
    errors: u64,
    requests_per_second: f64,
    duration_seconds: u64,
    concurrency: usize,
    frequency_hz: i32,
}

impl ProfileMetric {
    pub(super) fn new(
        config: &FlamegraphConfig,
        scenario: ProfileScenario,
        snapshot: &Snapshot,
    ) -> Self {
        Self {
            scenario: scenario.label(),
            requests: snapshot.total,
            errors: snapshot.errors,
            requests_per_second: snapshot.requests_per_second,
            duration_seconds: config.duration().as_secs(),
            concurrency: config.concurrency(scenario).get(),
            frequency_hz: config.frequency_hz(),
        }
    }
}

fn env_nonzero_u64(key: &str, default: u64) -> Result<NonZeroU64> {
    let value = match std::env::var(key) {
        Ok(raw) => raw
            .parse::<u64>()
            .with_context(|| format!("{key} must be a positive integer, got {raw:?}"))?,
        Err(std::env::VarError::NotPresent) => default,
        Err(std::env::VarError::NotUnicode(_)) => bail!("{key} must contain valid UTF-8"),
    };
    NonZeroU64::new(value).with_context(|| format!("{key} must be greater than zero"))
}

fn env_duration(key: &str, default: Duration) -> Result<Duration> {
    let raw = match std::env::var(key) {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => return Ok(default),
        Err(std::env::VarError::NotUnicode(_)) => bail!("{key} must contain valid UTF-8"),
    };
    parse_duration(key, &raw)
}

fn parse_duration(key: &str, raw: &str) -> Result<Duration> {
    let (value, multiplier) = raw
        .strip_suffix('s')
        .map(|value| (value, 1))
        .or_else(|| raw.strip_suffix('m').map(|value| (value, 60)))
        .with_context(|| format!("{key} must be a positive integer followed by s or m"))?;
    let seconds = value
        .parse::<u64>()
        .with_context(|| format!("{key} has invalid duration {raw:?}"))?
        .checked_mul(multiplier)
        .with_context(|| format!("{key} duration is too large"))?;
    let seconds =
        NonZeroU64::new(seconds).with_context(|| format!("{key} must be greater than zero"))?;
    Ok(Duration::from_secs(seconds.get()))
}

fn env_nonzero_usize(key: &str, default: usize) -> Result<NonZeroUsize> {
    let value = usize::try_from(env_nonzero_u64(key, default as u64)?.get())
        .with_context(|| format!("{key} does not fit in usize"))?;
    NonZeroUsize::new(value).with_context(|| format!("{key} must be greater than zero"))
}

pub(super) struct ProfileSession {
    scenario: ProfileScenario,
    output_path: PathBuf,
    guard: pprof::ProfilerGuard<'static>,
}

impl ProfileSession {
    pub(super) fn start(config: &FlamegraphConfig, scenario: ProfileScenario) -> Result<Self> {
        let guard = pprof::ProfilerGuard::new(config.frequency_hz())
            .with_context(|| format!("start {} profiler", scenario.label()))?;
        Ok(Self {
            scenario,
            output_path: config.output_path(scenario),
            guard,
        })
    }

    pub(super) fn finish(self) -> Result<()> {
        let report = self
            .guard
            .report()
            .build()
            .with_context(|| format!("build {} profile", self.scenario.label()))?;
        drop(self.guard);

        let parent = self.output_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create flamegraph directory {}", parent.display()))?;
        let file = File::create(&self.output_path).with_context(|| {
            format!(
                "create {} flamegraph {}",
                self.scenario.label(),
                self.output_path.display()
            )
        })?;
        report.flamegraph(file).with_context(|| {
            format!(
                "write {} flamegraph {}",
                self.scenario.label(),
                self.output_path.display()
            )
        })?;
        eprintln!(
            "[profile] wrote {} flamegraph to {}",
            self.scenario.label(),
            self.output_path.display()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenarios_have_stable_distinct_outputs() {
        assert_ne!(
            ProfileScenario::RedirectCached.file_name(),
            ProfileScenario::ApiMixed.file_name()
        );
    }

    #[test]
    fn default_configuration_is_valid() {
        let config = FlamegraphConfig::default();
        assert!(config.frequency_hz() > 0);
        assert!(!config.duration().is_zero());
        for &scenario in config.scenarios() {
            assert!(config.concurrency(scenario).get() > 0);
        }
    }

    #[test]
    fn scenario_parser_accepts_known_values() {
        assert_eq!(
            ProfileScenario::parse("redirect-cached").unwrap(),
            ProfileScenario::RedirectCached
        );
        assert_eq!(
            ProfileScenario::parse("api-mixed").unwrap(),
            ProfileScenario::ApiMixed
        );
        assert!(ProfileScenario::parse("unknown").is_err());
    }

    #[test]
    fn scenario_list_parser_accepts_all_and_ordered_subsets() {
        assert_eq!(
            parse_scenarios("TEST", "all").unwrap(),
            ProfileScenario::ALL
        );
        assert_eq!(
            parse_scenarios("TEST", "api-mixed, redirect-cached").unwrap(),
            [ProfileScenario::ApiMixed, ProfileScenario::RedirectCached]
        );
    }

    #[test]
    fn scenario_list_parser_rejects_empty_unknown_and_duplicate_values() {
        for raw in ["", "unknown", "redirect-cached,redirect-cached"] {
            assert!(parse_scenarios("TEST", raw).is_err(), "accepted {raw:?}");
        }
    }

    #[test]
    fn duration_parser_accepts_seconds_and_minutes() {
        assert_eq!(
            parse_duration("TEST_DURATION", "30s").unwrap(),
            Duration::from_secs(30)
        );
        assert_eq!(
            parse_duration("TEST_DURATION", "1m").unwrap(),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn duration_parser_rejects_invalid_values() {
        for raw in ["0s", "30", "1h", "seconds", "18446744073709551615m"] {
            assert!(
                parse_duration("TEST_DURATION", raw).is_err(),
                "accepted invalid duration {raw:?}"
            );
        }
    }
}
