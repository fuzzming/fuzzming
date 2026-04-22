use anyhow::Result;
use async_trait::async_trait;
use tokio::fs;

use crate::fuzzer::ports::inbound::FuzzerRunPort;
use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::fuzzer::use_cases::{evaluate_outcome, run_coverage, run_fuzzer};
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::fuzz_report::{FuzzOutcome, FuzzReport};

pub struct RunFuzzerUseCase {
    pub runner: Box<dyn TestRunnerPort>,
}

impl RunFuzzerUseCase {
    pub fn new(runner: Box<dyn TestRunnerPort>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl FuzzerRunPort for RunFuzzerUseCase {
    async fn run(&self, signal: RoundSignal) -> Result<FuzzReport> {
        let workspace = &signal.config.workspace_root;
        let fuzz_output_path = format!("{}/.fuzzming/fuzz_output.txt", workspace);
        let lcov_path = format!("{}/lcov.info", workspace);

        let fuzz_result = run_fuzzer("fuzzming", &*self.runner).await?;

        fs::create_dir_all(format!("{}/.fuzzming", workspace)).await?;
        fs::write(&fuzz_output_path, &fuzz_result.stdout).await?;

        let outcome = evaluate_outcome(&fuzz_result);

        let (outcome, lcov) = match outcome {
            FuzzOutcome::Pass => {
                run_coverage("coverage", &*self.runner).await?;
                (FuzzOutcome::Pass, Some(lcov_path))
            }
            other => (other, None),
        };

        Ok(FuzzReport { outcome })
    }
}
