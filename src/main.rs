mod interfaces;
mod orchestrator;
mod llm;
mod fuzzer;
mod reader;
mod executor;
mod reporter;
mod entry;
mod composition;

use anyhow::Result;
use entry::cli::cli_runner::CliRunner;

#[tokio::main]
async fn main() -> Result<()> {
    let runner = CliRunner::new();
    runner.run().await
}
