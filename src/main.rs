use anyhow::Result;
use fuzzming::entry::cli::cli_runner::CliRunner;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let runner = CliRunner::new();
    runner.run().await
}
