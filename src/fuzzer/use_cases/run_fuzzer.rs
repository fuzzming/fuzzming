use anyhow::Result;

use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::shared::models::RunnerResult;

pub async fn run_fuzzer(profile_name: &str, runner: &dyn TestRunnerPort) -> Result<RunnerResult> {
    runner.run_test(profile_name).await
}
