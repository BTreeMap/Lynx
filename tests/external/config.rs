use anyhow::{bail, Context, Result};
use reqwest::Url;
use std::env;
use std::num::NonZeroUsize;
use std::time::Duration;

const DEFAULT_API_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_REDIRECT_URL: &str = "http://127.0.0.1:3000";

/// A validated service base URL. Keeping path construction here prevents test
/// scenarios from accidentally mixing API and redirect origins.
#[derive(Clone, Debug)]
pub struct Endpoint {
    base: Url,
}

impl Endpoint {
    pub fn parse(value: &str, variable: &str) -> Result<Self> {
        let mut base = Url::parse(value)
            .with_context(|| format!("{variable} must be an absolute HTTP URL"))?;

        if !matches!(base.scheme(), "http" | "https") {
            bail!("{variable} must use http or https");
        }
        if base.query().is_some() || base.fragment().is_some() {
            bail!("{variable} must not contain a query string or fragment");
        }
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path().trim_end_matches('/'));
            base.set_path(&path);
        }

        Ok(Self { base })
    }

    pub fn url(&self, relative_path: &str) -> Result<Url> {
        self.base
            .join(relative_path)
            .with_context(|| format!("invalid path relative to {}", self.base))
    }

    pub fn as_str(&self) -> &str {
        self.base.as_str().trim_end_matches('/')
    }
}

/// Configuration shared by every black-box scenario.
///
/// The defaults make manual execution against a local service convenient, while
/// CI supplies only the values that differ from the normal loopback topology.
#[derive(Clone, Debug)]
pub struct ExternalConfig {
    pub api: Endpoint,
    pub redirect: Endpoint,
    pub request_timeout: Duration,
    pub readiness_timeout: Duration,
    pub concurrency: NonZeroUsize,
    pub container_name: Option<String>,
    pub analytics_expected: bool,
}

impl ExternalConfig {
    pub fn from_env() -> Result<Self> {
        let api_url = env::var("LYNX_E2E_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_owned());
        let redirect_url =
            env::var("LYNX_E2E_REDIRECT_URL").unwrap_or_else(|_| DEFAULT_REDIRECT_URL.to_owned());

        Ok(Self {
            api: Endpoint::parse(&api_url, "LYNX_E2E_API_URL")?,
            redirect: Endpoint::parse(&redirect_url, "LYNX_E2E_REDIRECT_URL")?,
            request_timeout: duration_from_env("LYNX_E2E_REQUEST_TIMEOUT_SECS", 15)?,
            readiness_timeout: duration_from_env("LYNX_E2E_READINESS_TIMEOUT_SECS", 45)?,
            concurrency: NonZeroUsize::new(usize_from_env("LYNX_E2E_CONCURRENCY", 100)?)
                .expect("validated non-zero concurrency"),
            container_name: env::var("LYNX_E2E_CONTAINER")
                .ok()
                .filter(|name| !name.trim().is_empty()),
            analytics_expected: bool_from_env("LYNX_E2E_EXPECT_ANALYTICS", false)?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StageConfig {
    pub concurrency: NonZeroUsize,
    pub duration: Duration,
    pub latency_sample_stride: NonZeroUsize,
}

impl StageConfig {
    pub fn new(concurrency: usize, duration: Duration) -> Result<Self> {
        let concurrency = NonZeroUsize::new(concurrency)
            .context("traffic stage concurrency must be greater than zero")?;
        if duration.is_zero() {
            bail!("traffic stage duration must be greater than zero");
        }

        Ok(Self {
            concurrency,
            duration,
            // Sampling one request in 256 bounds collector contention and memory
            // use while keeping percentile samples representative at high RPS.
            latency_sample_stride: NonZeroUsize::new(256).expect("constant is non-zero"),
        })
    }
}

pub fn duration_from_env(name: &str, default_seconds: u64) -> Result<Duration> {
    let seconds = match env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .with_context(|| format!("{name} must be an integer number of seconds"))?,
        Err(_) => default_seconds,
    };
    if seconds == 0 {
        bail!("{name} must be greater than zero");
    }
    Ok(Duration::from_secs(seconds))
}

pub fn usize_from_env(name: &str, default: usize) -> Result<usize> {
    let value = match env::var(name) {
        Ok(value) => value
            .parse::<usize>()
            .with_context(|| format!("{name} must be a positive integer"))?,
        Err(_) => default,
    };
    if value == 0 {
        bail!("{name} must be greater than zero");
    }
    Ok(value)
}

pub fn bool_from_env(name: &str, default: bool) -> Result<bool> {
    let value = match env::var(name) {
        Ok(value) => value,
        Err(_) => return Ok(default),
    };

    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        _ => bail!("{name} must be one of true, false, 1, 0, yes, or no"),
    }
}
