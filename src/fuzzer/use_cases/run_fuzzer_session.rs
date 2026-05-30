use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use async_trait::async_trait;

use crate::fuzzer::ports::inbound::FuzzerRunPort;
use crate::fuzzer::ports::outbound::{FuzzerOutputPort, TestRunnerPort};
use crate::fuzzer::use_cases::{enrich_coverage_context, run_coverage, run_fuzzer};
use crate::reader::use_cases::parse_lcov::parse_lcov;
use crate::shared::models::{BugInfo, RunnerResult};
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::fuzz_report::{FuzzOutcome, FuzzReport};

pub struct RunFuzzerUseCase {
    pub runner: Box<dyn TestRunnerPort>,
    pub output: Box<dyn FuzzerOutputPort>,
    pub workspace_root: PathBuf,
}

impl RunFuzzerUseCase {
    pub fn new(
        runner: Box<dyn TestRunnerPort>,
        output: Box<dyn FuzzerOutputPort>,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            runner,
            output,
            workspace_root,
        }
    }

    async fn process_results(
        &self,
        signals: &[RoundSignal],
        fuzz_result: &RunnerResult,
        erroring: &HashSet<String>,
        error_result: &RunnerResult,
    ) -> Result<Vec<FuzzReport>> {
        let mut reports: Vec<FuzzReport> = Vec::with_capacity(signals.len());
        let mut any_pass = false;
        let compile_error = is_compile_error(fuzz_result);

        for signal in signals {
            let contract = &signal.contract_name;

            let (contract_output, outcome, bugs) = if erroring.contains(contract) {
                let details = compile_error_details(&error_result.stdout, &error_result.stderr);
                let msg = format!(
                    "COMPILATION ERROR — fix the Solidity before fuzzing can proceed:\n{}",
                    details
                );
                (msg, FuzzOutcome::CompileError, vec![])
            } else if compile_error {
                let details = compile_error_details(&fuzz_result.stdout, &fuzz_result.stderr);
                let msg = format!(
                    "COMPILATION ERROR — fix the Solidity before fuzzing can proceed:\n{}",
                    details
                );
                (msg, FuzzOutcome::CompileError, vec![])
            } else {
                let filtered = self.runner.filter_output(&fuzz_result.stdout, contract);
                let (mut outcome, bugs) =
                    evaluate_outcome_for_contract(&*self.runner, fuzz_result, contract);
                // If forge skipped this contract, treat it as CompileError to avoid false DevTestFailed.
                if matches!(outcome, FuzzOutcome::DevTestFailed)
                    && filtered.is_empty()
                    && fuzz_result.stdout.contains("InvariantTest")
                {
                    outcome = FuzzOutcome::CompileError;
                }
                // setUp reverts silently: forge exits 0 but every invariant ran 0 calls.
                let setup_failed =
                    matches!(outcome, FuzzOutcome::Pass) && all_invariants_zero_calls(&filtered);
                if setup_failed {
                    outcome = FuzzOutcome::CompileError;
                }
                let output = if setup_failed {
                    format!(
                        "SETUP FAILURE — setUp() is reverting silently. Every invariant ran \
                         with 0 calls, meaning the fuzzer never exercised the contract.\n\
                         Check that setUp() deploys all mocks without reverting, creates the \
                         target contract, and calls targetContract(address(handler)).\n\
                         Forge output:\n{filtered}"
                    )
                } else if matches!(outcome, FuzzOutcome::CompileError) && filtered.is_empty() {
                    let details = compile_error_details(&fuzz_result.stdout, &fuzz_result.stderr);
                    format!(
                        "COMPILATION ERROR — fix the Solidity before fuzzing can proceed:\n{}",
                        details
                    )
                } else if matches!(outcome, FuzzOutcome::DevTestFailed) && filtered.is_empty() {
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

            self.output
                .write_fuzz_output(contract, &contract_output)
                .await?;
            if matches!(outcome, FuzzOutcome::Pass) {
                any_pass = true;
            }
            reports.push(FuzzReport {
                outcome,
                bugs,
                lcov_path: None,
            });
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
                        if let Ok(mut ctx) = parse_lcov(&filtered) {
                            enrich_coverage_context(&mut ctx, &self.workspace_root).await;
                            self.output.write_coverage_context(contract, &ctx).await?;
                        }
                    }
                }
            }
        }

        Ok(reports)
    }
}

#[async_trait]
impl FuzzerRunPort for RunFuzzerUseCase {
    async fn run(&self, signals: Vec<RoundSignal>) -> Result<Vec<FuzzReport>> {
        if signals.is_empty() {
            return Err(anyhow::anyhow!("fuzzer called with no signals"));
        }

        // Restore any dirs left disabled by a previous crashed session.
        restore_leftover_disabled(&self.workspace_root).await;

        let fuzz_result = run_fuzzer("fuzzming", &*self.runner).await?;

        if is_compile_error(&fuzz_result) {
            let erroring =
                extract_erroring_contract_names(&fuzz_result.stderr, &fuzz_result.stdout);
            let has_healthy = signals.iter().any(|s| !erroring.contains(&s.contract_name));

            if !erroring.is_empty() && has_healthy {
                // Move erroring test dirs outside the forge scan tree.
                let stash = self.workspace_root.join(".fuzzming-disabled");
                let _ = tokio::fs::create_dir_all(&stash).await;

                let mut disabled: Vec<(PathBuf, PathBuf)> = Vec::new();
                for name in &erroring {
                    let original = self.workspace_root.join("test").join("fuzzming").join(name);
                    let hidden = stash.join(name);
                    if original.exists() {
                        let _ = tokio::fs::rename(&original, &hidden).await;
                        disabled.push((hidden, original));
                    }
                }

                // Re-run forge for the healthy contracts (erroring dirs stashed).
                let healthy_result = run_fuzzer("fuzzming", &*self.runner).await;
                let reports = match healthy_result {
                    Ok(result) => {
                        self.process_results(&signals, &result, &erroring, &fuzz_result)
                            .await
                    }
                    Err(e) => Err(e),
                };

                // Always restore before returning.
                for (hidden, original) in &disabled {
                    if hidden.exists() {
                        let _ = tokio::fs::rename(hidden, original).await;
                    }
                }

                return reports;
            }
        }

        // No compile error, or all contracts are erroring — proceed with standard evaluation.
        let empty = HashSet::new();
        self.process_results(&signals, &fuzz_result, &empty, &fuzz_result)
            .await
    }
}

async fn restore_leftover_disabled(workspace_root: &Path) {
    let stash = workspace_root.join(".fuzzming-disabled");
    let mut dir = match tokio::fs::read_dir(&stash).await {
        Ok(d) => d,
        Err(_) => return,
    };
    while let Ok(Some(entry)) = dir.next_entry().await {
        let hidden = entry.path();
        let name = entry.file_name();
        let original = workspace_root.join("test").join("fuzzming").join(&name);
        if hidden.is_dir() && !original.exists() {
            let _ = tokio::fs::rename(&hidden, &original).await;
        }
    }
}

fn extract_erroring_contract_names(stderr: &str, stdout: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in stderr.lines().chain(stdout.lines()) {
        // Match lines like: --> test/fuzzming/ContractName/File.sol:10:5:
        if let Some(pos) = line.find("test/fuzzming/") {
            let rest = &line[pos + "test/fuzzming/".len()..];
            if let Some(slash) = rest.find('/') {
                let name = rest[..slash].trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names
}

/// Forge emits detailed solc errors to stdout, not stderr.
/// Return stdout when it has diagnostic content, else fall back to stderr.
fn compile_error_details<'a>(stdout: &'a str, stderr: &'a str) -> &'a str {
    if stdout.contains("Error (")
        || stdout.contains("Compiler run failed")
        || stdout.contains("TypeError")
    {
        stdout
    } else {
        stderr
    }
}

fn is_compile_error(result: &RunnerResult) -> bool {
    result.exit_code != 0
        && (result.stderr.contains("Compiler run failed")
            || result.stderr.contains("error[")
            || result.stdout.contains("Compiler run failed")
            || result.stdout.contains("error["))
}

fn evaluate_outcome_for_contract(
    runner: &dyn TestRunnerPort,
    result: &RunnerResult,
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

/// Returns true when the filtered forge output contains at least one invariant
/// result line AND every one of them shows `calls: 0` — the signature of a
/// silently-reverting setUp().
fn all_invariants_zero_calls(output: &str) -> bool {
    let mut found_any = false;
    for line in output.lines() {
        if !line.contains("(runs:") || !line.contains(", calls:") {
            continue;
        }
        found_any = true;
        // Extract the number after "calls: "
        let calls_nonzero = line
            .find(", calls: ")
            .and_then(|pos| {
                line[pos + 9..]
                    .split(',')
                    .next()
                    .and_then(|s| s.trim().parse::<u64>().ok())
            })
            .map(|n| n > 0)
            .unwrap_or(true); // Default to non-zero on parse failure to prevent false positives.
        if calls_nonzero {
            return false;
        }
    }
    found_any
}
