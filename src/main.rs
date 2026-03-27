use anyhow::Result;
use fuzzming::entry::cli::cli_runner::CliRunner;

#[tokio::main]
async fn main() -> Result<()> {
    let runner = CliRunner::new();
    runner.run().await
}
