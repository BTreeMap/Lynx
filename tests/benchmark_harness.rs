#[path = "external/mod.rs"]
mod external;

use anyhow::Result;

#[tokio::test]
#[ignore = "runs an externally hosted native Rust benchmark; see tests/README.md"]
async fn native_external_benchmark() -> Result<()> {
    external::benchmark::run_from_env().await.map(|_| ())
}
