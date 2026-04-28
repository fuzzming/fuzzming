use anyhow::Result;
use async_trait::async_trait;
use tokio::fs;

use crate::fuzzer::ports::inbound::FuzzerRunPort;
use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::fuzzer::use_cases::{run_coverage, run_fuzzer};
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::fuzz_report::{FuzzOutcome, FuzzReport};

const FUZZMING_DIR: &str = ".fuzzming";

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
    async fn run(&self, signals: Vec<RoundSignal>) -> Result<Vec<FuzzReport>> {
        let workspace = signals
            .first()
            .ok_or_else(|| anyhow::anyhow!("fuzzer called with no signals"))?
            .config
            .workspace_root
            .clone();

        // One forge test run covers all contracts.
        let fuzz_result = run_fuzzer("fuzzming", &*self.runner).await?;

        // Evaluate and write per-contract fuzz output.
        let mut reports: Vec<FuzzReport> = Vec::with_capacity(signals.len());
        let mut any_pass = false;

        for signal in &signals {
            let contract = &signal.contract_name;
            let contract_dir = workspace.join(FUZZMING_DIR).join(contract);
            fs::create_dir_all(&contract_dir).await?;

            let contract_output = filter_output_for_contract(&fuzz_result.stdout, contract);
            fs::write(contract_dir.join("fuzz_output.txt"), &contract_output).await?;

            let outcome = evaluate_outcome_for_contract(&fuzz_result, contract);
            if matches!(outcome, FuzzOutcome::Pass) {
                any_pass = true;
            }
            reports.push(FuzzReport { outcome, lcov_path: None });
        }

        // One forge coverage run when at least one contract passed.
        if any_pass {
            run_coverage("coverage", &*self.runner).await?;
            let lcov_src = workspace.join("lcov.info");

            for (signal, report) in signals.iter().zip(reports.iter_mut()) {
                if matches!(report.outcome, FuzzOutcome::Pass) {
                    let contract = &signal.contract_name;
                    let contract_dir = workspace.join(FUZZMING_DIR).join(contract);
                    let lcov_dest = contract_dir.join("lcov.info");

                    let lcov_content = fs::read_to_string(&lcov_src).await?;
                    let filtered = filter_lcov_for_contract(&lcov_content, contract);
                    fs::write(&lcov_dest, filtered).await?;

                    report.lcov_path = Some(lcov_dest);
                }
            }
        }

        Ok(reports)
    }
}

/// Keep only lines from forge test stdout that mention a specific contract.
fn filter_output_for_contract(stdout: &str, contract_name: &str) -> String {
    let invariant_test = format!("{}InvariantTest", contract_name);
    stdout
        .lines()
        .filter(|line| line.contains(&invariant_test))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Evaluate the outcome of a single contract from the combined forge output.
fn evaluate_outcome_for_contract(result: &crate::shared::models::RunnerResult, contract_name: &str) -> FuzzOutcome {
    let invariant_test = format!("{}InvariantTest", contract_name);
    let combined = format!("{}\n{}", result.stdout, result.stderr);

    // Check for failure lines that mention this contract's invariant test.
    for line in combined.lines() {
        if line.contains("[FAIL") && line.contains(&invariant_test) {
            if line.contains("invariant_") {
                return FuzzOutcome::Bug;
            }
            return FuzzOutcome::DevTestFailed;
        }
    }

    // If forge exited non-zero but no failure line for this contract, it passed locally.
    // If forge exited zero, everything passed.
    FuzzOutcome::Pass
}

/// Extract only the LCOV records for a given contract.
/// LCOV groups records by source file (SF:). We keep every record whose SF: line
/// contains the contract name.
fn filter_lcov_for_contract(lcov: &str, contract_name: &str) -> String {
    let mut keep = false;
    let mut out = Vec::<&str>::new();

    for line in lcov.lines() {
        if line.starts_with("SF:") {
            keep = line.contains(contract_name);
        }
        if keep {
            out.push(line);
        }
    }
    out.join("\n")
}
