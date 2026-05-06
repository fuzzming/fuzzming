use anyhow::Result;
use async_trait::async_trait;

use crate::fuzzer::ports::inbound::FuzzerRunPort;
use crate::fuzzer::ports::outbound::{FuzzerOutputPort, TestRunnerPort};
use crate::fuzzer::use_cases::{run_coverage, run_fuzzer};
use crate::shared::models::BugInfo;
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::fuzz_report::{FuzzOutcome, FuzzReport};

pub struct RunFuzzerUseCase {
    pub runner: Box<dyn TestRunnerPort>,
    pub output: Box<dyn FuzzerOutputPort>,
}

impl RunFuzzerUseCase {
    pub fn new(runner: Box<dyn TestRunnerPort>, output: Box<dyn FuzzerOutputPort>) -> Self {
        Self { runner, output }
    }
}

#[async_trait]
impl FuzzerRunPort for RunFuzzerUseCase {
    async fn run(&self, signals: Vec<RoundSignal>) -> Result<Vec<FuzzReport>> {
        if signals.is_empty() {
            return Err(anyhow::anyhow!("fuzzer called with no signals"));
        }

        let fuzz_result = run_fuzzer("fuzzming", &*self.runner).await?;

        let mut reports: Vec<FuzzReport> = Vec::with_capacity(signals.len());
        let mut any_pass = false;
        let compile_error = is_compile_error(&fuzz_result);

        for signal in &signals {
            let contract = &signal.contract_name;

            let (contract_output, outcome, bugs) = if compile_error {
                let msg = format!(
                    "COMPILATION ERROR — fix the Solidity before fuzzing can proceed:\n{}",
                    fuzz_result.stderr
                );
                (msg, FuzzOutcome::CompileError, vec![])
            } else {
                let filtered = self.runner.filter_output(&fuzz_result.stdout, contract);
                let (outcome, bugs) =
                    evaluate_outcome_for_contract(&*self.runner, &fuzz_result, contract);
                // For DevTestFailed the error is outside the section markers so filter_output
                // returns empty. Fall back to stderr + full stdout so the LLM sees the error.
                let output = if matches!(outcome, FuzzOutcome::DevTestFailed) && filtered.is_empty()
                {
                    let mut msg = String::from("TEST FAILED — fix the handler/invariant test:\n");
                    if !fuzz_result.stderr.is_empty() {
                        msg.push_str(&fuzz_result.stderr);
                    }
                    if !fuzz_result.stdout.is_empty() {
                        msg.push('\n');
                        msg.push_str(&fuzz_result.stdout);
                    }
                    msg
                } else {
                    filtered
                };
                (output, outcome, bugs)
            };

            self.output.write_fuzz_output(contract, &contract_output).await?;
            if matches!(outcome, FuzzOutcome::Pass) {
                any_pass = true;
            }
            reports.push(FuzzReport { outcome, bugs, lcov_path: None });
        }

        if any_pass {
            let coverage = run_coverage("coverage", &*self.runner).await?;

            if let Some(lcov_content) = coverage.lcov_content {
                for (signal, report) in signals.iter().zip(reports.iter_mut()) {
                    if matches!(report.outcome, FuzzOutcome::Pass) {
                        let contract = &signal.contract_name;
                        let filtered = self.runner.filter_lcov(&lcov_content, contract);
                        let path = self.output.write_lcov(contract, &filtered).await?;
                        report.lcov_path = Some(path);
                    }
                }
            }
        }

        Ok(reports)
    }
}

fn is_compile_error(result: &crate::shared::models::RunnerResult) -> bool {
    result.exit_code != 0
        && (result.stderr.contains("Compiler run failed")
            || result.stderr.contains("error[")
            || result.stdout.contains("Compiler run failed")
            || result.stdout.contains("error["))
}

fn evaluate_outcome_for_contract(
    runner: &dyn TestRunnerPort,
    result: &crate::shared::models::RunnerResult,
    contract_name: &str,
) -> (FuzzOutcome, Vec<BugInfo>) {
    if result.exit_code == 0 {
        return (FuzzOutcome::Pass, vec![]);
    }

    let bugs = runner.collect_bugs(&result.stdout, contract_name);
    if !bugs.is_empty() {
        return (FuzzOutcome::Bug, bugs);
    }

    (FuzzOutcome::DevTestFailed, vec![])
}
