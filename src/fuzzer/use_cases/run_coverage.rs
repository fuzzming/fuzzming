use anyhow::Result;

use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::shared::models::CoverageResult;

pub async fn run_coverage(profile_name: &str, runner: &dyn TestRunnerPort) -> Result<CoverageResult> {
    runner.run_coverage(profile_name).await
}
