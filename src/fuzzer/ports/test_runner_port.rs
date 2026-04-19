use crate::shared::models::RunnerResult;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait TestRunnerPort: Send + Sync {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult>;
    async fn run_coverage(&self, profile_name: &str) -> Result<RunnerResult>;
}
