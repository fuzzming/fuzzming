use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::artifacts::RunnerResult;

#[async_trait]
pub trait TestRunnerPort: Send + Sync {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult>;
    async fn run_coverage(&self, profile_name: &str) -> Result<RunnerResult>;
}
