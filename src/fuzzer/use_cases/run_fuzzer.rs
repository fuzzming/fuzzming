use anyhow::Result;

use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::shared::models::RunnerResult;

pub async fn run_fuzzer(profile_name: &str, runner: &dyn TestRunnerPort) -> Result<RunnerResult> {
    // Pre-flight: compile only first. If solc errors exist, return them immediately
    // without spending time on forge test. This turns a wasted round into instant feedback.
    let build = runner.run_build(profile_name).await?;
    if build.exit_code != 0
        && (build.stderr.contains("Compiler run failed")
            || build.stderr.contains("error[")
            || build.stdout.contains("Compiler run failed")
            || build.stdout.contains("error["))
    {
        return Ok(build);
    }

    runner.run_test(profile_name).await
}
