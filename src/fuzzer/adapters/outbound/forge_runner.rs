use std::path::PathBuf;

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::shared::models::RunnerResult;

pub struct ForgeRunner {
    pub working_dir: PathBuf,
}

impl ForgeRunner {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

fn forge_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let current = std::env::var("PATH").unwrap_or_default();
    format!("{home}/.foundry/bin:{current}")
}

#[async_trait]
impl TestRunnerPort for ForgeRunner {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult> {
        let output = tokio::process::Command::new("forge")
            .args(["test"])
            .env("FOUNDRY_PROFILE", profile_name)
            .env("PATH", forge_path())
            .current_dir(&self.working_dir)
            .output()
            .await
            .context("failed to spawn `forge test`")?;

        Ok(RunnerResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    async fn run_coverage(&self, profile_name: &str) -> Result<RunnerResult> {
        let output = tokio::process::Command::new("forge")
            .args(["coverage", "--report", "lcov"])
            .env("FOUNDRY_PROFILE", profile_name)
            .env("PATH", forge_path())
            .current_dir(&self.working_dir)
            .output()
            .await
            .context("failed to spawn `forge coverage`")?;

        Ok(RunnerResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
