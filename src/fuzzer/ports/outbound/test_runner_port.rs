use anyhow::Result;
use async_trait::async_trait;

use crate::shared::models::{BugInfo, CoverageResult, RunnerResult};

#[async_trait]
pub trait TestRunnerPort: Send + Sync {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult>;
    async fn run_build(&self, profile_name: &str) -> Result<RunnerResult>;
    async fn run_coverage(&self, profile_name: &str) -> Result<CoverageResult>;

    fn collect_bugs(&self, stdout: &str, contract_name: &str) -> Vec<BugInfo>;
    fn filter_output(&self, stdout: &str, contract_name: &str) -> String;
    fn filter_lcov(&self, lcov: &str, contract_name: &str) -> String;
}
