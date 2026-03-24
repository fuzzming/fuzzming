use anyhow::Result;
use crate::interfaces::artifacts::RunnerResult;
use crate::fuzzer::ports::TestRunnerPort;

pub async fn run_coverage(
    profile_name: &str,
    runner: &dyn TestRunnerPort,
) -> Result<RunnerResult> {
    todo!()
}
