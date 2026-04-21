use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::shared::models::RunnerResult;
use anyhow::Result;

pub async fn run_coverage(profile_name: &str, runner: &dyn TestRunnerPort) -> Result<RunnerResult> {
    todo!()
}
