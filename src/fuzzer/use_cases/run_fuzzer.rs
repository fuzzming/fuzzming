use crate::fuzzer::ports::TestRunnerPort;
use crate::shared::models::RunnerResult;
use anyhow::Result;

pub async fn run_fuzzer(profile_name: &str, runner: &dyn TestRunnerPort) -> Result<RunnerResult> {
    todo!()
}
