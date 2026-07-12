#![allow(clippy::struct_field_names)]

#[path = "external/mod.rs"]
mod external;

use anyhow::Result;

#[tokio::test]
#[ignore = "requires a running Docker-backed Lynx service; see tests/README.md"]
async fn external_functional_and_concurrency_suite() -> Result<()> {
    let config = external::config::ExternalConfig::from_env()?;
    let service = external::client::ExternalService::new(&config)?;

    external::scenarios::run_functional_suite(&service, &config).await?;
    external::scenarios::run_concurrency_suite(&service, &config).await
}

#[tokio::test]
#[ignore = "requires LYNX_E2E_CONTAINER and a running Docker-backed Lynx service"]
async fn external_graceful_shutdown_suite() -> Result<()> {
    let config = external::config::ExternalConfig::from_env()?;
    let service = external::client::ExternalService::new(&config)?;
    external::lifecycle::run_shutdown_suite(&service, &config).await
}
