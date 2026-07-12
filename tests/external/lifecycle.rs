use anyhow::{bail, ensure, Context, Result};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

use super::client::ExternalService;
use super::config::ExternalConfig;
use super::scenarios::{
    run_redirect_burst, wait_for_analytics_exact, wait_for_clicks_exact, CodeFactory,
};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);
const REDIRECTS_BEFORE_SIGNAL: usize = 128;

#[derive(Clone, Copy, Debug)]
pub enum ShutdownSignal {
    Terminate,
    Interrupt,
}

impl ShutdownSignal {
    fn docker_name(self) -> &'static str {
        match self {
            Self::Terminate => "SIGTERM",
            Self::Interrupt => "SIGINT",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Terminate => "sigterm",
            Self::Interrupt => "sigint",
        }
    }
}

pub async fn run_shutdown_suite(service: &ExternalService, config: &ExternalConfig) -> Result<()> {
    service.wait_until_ready(config.readiness_timeout).await?;
    let container = config
        .container_name
        .as_deref()
        .context("LYNX_E2E_CONTAINER is required for lifecycle tests")?;
    let auth = service.auth_mode().await?;
    let codes = CodeFactory::new(auth.short_code_max_length)?;

    for signal in [ShutdownSignal::Terminate, ShutdownSignal::Interrupt] {
        run_shutdown_cycle(service, config, container, signal, &codes).await?;
    }
    Ok(())
}

async fn run_shutdown_cycle(
    service: &ExternalService,
    config: &ExternalConfig,
    container: &str,
    signal: ShutdownSignal,
    codes: &CodeFactory,
) -> Result<()> {
    let code = codes.code();
    service
        .create(
            &format!("https://example.com/lifecycle/{}", signal.name()),
            Some(&code),
        )
        .await?;
    ensure!(
        service.get(&code).await?.clicks == 0,
        "new lifecycle URL is not empty"
    );

    // Every successful redirect has synchronously transferred ownership of its
    // increment to the in-memory counter before the HTTP response is emitted.
    // Signalling only after this burst makes the post-restart count exact.
    run_redirect_burst(service, &code, REDIRECTS_BEFORE_SIGNAL).await?;

    docker(&["kill", "--signal", signal.docker_name(), container]).await?;
    wait_for_stopped(container, SHUTDOWN_TIMEOUT).await?;
    ensure!(
        docker(&["inspect", "--format", "{{.State.ExitCode}}", container])
            .await?
            .trim()
            == "0",
        "container exited unsuccessfully after {}",
        signal.docker_name()
    );
    ensure!(
        docker(&["logs", container])
            .await?
            .contains("Shutdown complete"),
        "container logs do not confirm graceful shutdown after {}",
        signal.docker_name()
    );

    docker(&["start", container]).await?;
    service.wait_until_ready(config.readiness_timeout).await?;
    wait_for_clicks_exact(
        service,
        &code,
        REDIRECTS_BEFORE_SIGNAL as i64,
        SHUTDOWN_TIMEOUT,
    )
    .await
    .with_context(|| format!("exact click durability after {}", signal.docker_name()))?;

    if config.analytics_expected {
        wait_for_analytics_exact(
            service,
            &code,
            REDIRECTS_BEFORE_SIGNAL as i64,
            SHUTDOWN_TIMEOUT,
        )
        .await
        .with_context(|| format!("exact analytics durability after {}", signal.docker_name()))?;
    }

    Ok(())
}

async fn wait_for_stopped(container: &str, timeout: Duration) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        let running = docker(&["inspect", "--format", "{{.State.Running}}", container]).await?;
        if running.trim() == "false" {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    bail!("container {container} did not stop within {timeout:?}")
}

async fn docker(arguments: &[&str]) -> Result<String> {
    let output = Command::new("docker")
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .await
        .with_context(|| format!("run docker {}", arguments.join(" ")))?;
    if !output.status.success() {
        bail!(
            "docker {} failed with {}: {}",
            arguments.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("docker command produced non-UTF-8 output")
}
