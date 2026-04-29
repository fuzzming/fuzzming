use std::collections::HashSet;

use anyhow::Result;
use async_trait::async_trait;
use tokio::fs;

use crate::fuzzer::ports::inbound::FuzzerRunPort;
use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::fuzzer::use_cases::{run_coverage, run_fuzzer};
use crate::shared::models::BugInfo;
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

        let fuzz_result = run_fuzzer("fuzzming", &*self.runner).await?;

        let mut reports: Vec<FuzzReport> = Vec::with_capacity(signals.len());
        let mut any_pass = false;

        for signal in &signals {
            let contract = &signal.contract_name;
            let contract_dir = workspace.join(FUZZMING_DIR).join(contract);
            fs::create_dir_all(&contract_dir).await?;

            let contract_output = filter_output_for_contract(&fuzz_result.stdout, contract);
            fs::write(contract_dir.join("fuzz_output.txt"), &contract_output).await?;

            let (outcome, bugs) = evaluate_outcome_for_contract(&fuzz_result, contract);
            if matches!(outcome, FuzzOutcome::Pass) {
                any_pass = true;
            }
            reports.push(FuzzReport { outcome, bugs, lcov_path: None });
        }

        if any_pass {
            run_coverage("coverage", &*self.runner).await?;
            let lcov_src = workspace.join("lcov.info");

            if let Ok(lcov_content) = fs::read_to_string(&lcov_src).await {
                for (signal, report) in signals.iter().zip(reports.iter_mut()) {
                    if matches!(report.outcome, FuzzOutcome::Pass) {
                        let contract = &signal.contract_name;
                        let contract_dir = workspace.join(FUZZMING_DIR).join(contract);
                        let lcov_dest = contract_dir.join("lcov.info");
                        let filtered = filter_lcov_for_contract(&lcov_content, contract);
                        fs::write(&lcov_dest, filtered).await?;
                        report.lcov_path = Some(lcov_dest);
                    }
                }
            }
        }

        Ok(reports)
    }
}

/// Returns the outcome and all bugs found for a single contract.
fn evaluate_outcome_for_contract(
    result: &crate::shared::models::RunnerResult,
    contract_name: &str,
) -> (FuzzOutcome, Vec<BugInfo>) {
    if result.exit_code == 0 {
        return (FuzzOutcome::Pass, vec![]);
    }

    let bugs = collect_bugs_for_contract(&result.stdout, contract_name);
    if !bugs.is_empty() {
        return (FuzzOutcome::Bug, bugs);
    }

    (FuzzOutcome::DevTestFailed, vec![])
}

/// Parse all failing invariants for a contract out of forge stdout.
///
/// Forge output format (empirically verified — both tokens are on separate lines):
///
///   [FAIL: assertion message]
///     [Sequence] (original: N, shrunk: M)
///       sender=0x... calldata=handler_foo() args=[]
///    invariant_name() (runs: 1, calls: 1, reverts: 0)
///
/// The reliable signal for a failing invariant is a line containing both
/// `invariant_` and `(runs:`. The `[Sequence]` block preceding it is the
/// call sequence needed for reproduction.
///
/// Forge prints results twice (main body + "Failing tests:" summary).
/// We stop at the summary to avoid duplicates.
fn collect_bugs_for_contract(stdout: &str, contract_name: &str) -> Vec<BugInfo> {
    let section_marker = format!("{}InvariantTest", contract_name);
    let mut in_section = false;
    let mut in_fail_block = false;
    let mut collecting_sequence = false;
    let mut sequence_lines: Vec<&str> = Vec::new();
    let mut bugs: Vec<BugInfo> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for line in stdout.lines() {
        // Stop at the summary block to avoid processing results twice.
        if line.trim_start().starts_with("Failing tests:") {
            break;
        }

        // Enter this contract's section.
        if line.contains(&section_marker) {
            in_section = true;
            continue;
        }

        // Exit if another contract's section starts.
        if in_section && line.contains("InvariantTest") && !line.contains(&section_marker) {
            break;
        }

        if !in_section {
            continue;
        }

        // A new [FAIL block begins.
        if line.trim_start().starts_with("[FAIL") {
            in_fail_block = true;
            collecting_sequence = false;
            sequence_lines.clear();
            continue;
        }

        if !in_fail_block {
            continue;
        }

        // Sequence header line.
        if line.contains("[Sequence]") || line.contains("[Shrunk sequence]") {
            collecting_sequence = true;
            continue;
        }

        // Box-drawing characters mark the call stats table — sequence is over.
        if line.contains('╭') || line.contains('╰') || line.contains('├') {
            collecting_sequence = false;
            continue;
        }

        // Invariant result line — ends the block and gives us the name.
        if line.contains("invariant_") && line.contains("(runs:") {
            if let Some(name) = extract_invariant_name(line) {
                if seen.insert(name.clone()) {
                    bugs.push(BugInfo {
                        invariant_name: name,
                        call_sequence: sequence_lines.join("\n"),
                    });
                }
            }
            in_fail_block = false;
            collecting_sequence = false;
            sequence_lines.clear();
            continue;
        }

        // Collect sequence lines (skip table rows and blank lines).
        if collecting_sequence && !line.trim().is_empty()
            && !line.contains('│') && !line.contains('╰')
        {
            sequence_lines.push(line.trim());
        }
    }

    bugs
}

fn extract_invariant_name(line: &str) -> Option<String> {
    let start = line.find("invariant_")?;
    let rest = &line[start..];
    let end = rest.find('(').unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

/// Capture the full forge output section for one contract (from its header
/// to the next contract's header or the "Failing tests" summary).
fn filter_output_for_contract(stdout: &str, contract_name: &str) -> String {
    let section_marker = format!("{}InvariantTest", contract_name);
    let mut in_section = false;
    let mut lines: Vec<&str> = Vec::new();

    for line in stdout.lines() {
        if line.contains("Failing tests:") {
            break;
        }

        if line.contains(&section_marker) {
            in_section = true;
        } else if in_section && line.contains("InvariantTest") {
            break;
        }

        if in_section {
            lines.push(line);
        }
    }

    lines.join("\n")
}

/// Extract only the LCOV records for a given contract.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::models::RunnerResult;

    fn result(exit_code: i32, stdout: &str) -> RunnerResult {
        RunnerResult { exit_code, stdout: stdout.to_string(), stderr: String::new() }
    }

    // Forge output format confirmed empirically: [FAIL and invariant_ on separate lines.
    const MULTI_BUG_OUTPUT: &str = "\
Ran 3 tests for test/invariants/VaultInvariantTest.sol:VaultInvariantTest
[FAIL: count should never exceed 100: 1000 > 100]
\t[Sequence] (original: 1, shrunk: 1)
\t\tsender=0xAAA calldata=handler_bigJump() args=[]
 invariant_bounded() (runs: 1, calls: 1, reverts: 0)
[FAIL: count must be at least 1: 0 < 1]
\t[Sequence] (original: 1, shrunk: 1)
\t\tsender=0xBBB calldata=handler_reset() args=[]
 invariant_in_range() (runs: 1, calls: 1, reverts: 0)
[FAIL: count should never be zero: 0 <= 0]
\t[Sequence] (original: 1, shrunk: 1)
\t\tsender=0xCCC calldata=handler_reset() args=[]
 invariant_never_zero() (runs: 1, calls: 1, reverts: 0)
Suite result: FAILED. 0 passed; 3 failed;
Failing tests:
Encountered 3 failing tests in test/invariants/VaultInvariantTest.sol:VaultInvariantTest
[FAIL: count should never exceed 100: 1000 > 100]
 invariant_bounded() (runs: 1, calls: 1, reverts: 0)";

    #[test]
    fn detects_all_three_bugs() {
        let r = result(1, MULTI_BUG_OUTPUT);
        let (outcome, bugs) = evaluate_outcome_for_contract(&r, "Vault");
        assert!(matches!(outcome, FuzzOutcome::Bug));
        assert_eq!(bugs.len(), 3);
        assert!(bugs.iter().any(|b| b.invariant_name == "invariant_bounded"));
        assert!(bugs.iter().any(|b| b.invariant_name == "invariant_in_range"));
        assert!(bugs.iter().any(|b| b.invariant_name == "invariant_never_zero"));
    }

    #[test]
    fn call_sequence_extracted_per_bug() {
        let r = result(1, MULTI_BUG_OUTPUT);
        let (_, bugs) = evaluate_outcome_for_contract(&r, "Vault");
        let bounded = bugs.iter().find(|b| b.invariant_name == "invariant_bounded").unwrap();
        assert!(bounded.call_sequence.contains("handler_bigJump"), "expected bigJump in sequence");
        let never_zero = bugs.iter().find(|b| b.invariant_name == "invariant_never_zero").unwrap();
        assert!(never_zero.call_sequence.contains("handler_reset"), "expected reset in sequence");
    }

    #[test]
    fn no_duplicate_bugs_from_summary_section() {
        let r = result(1, MULTI_BUG_OUTPUT);
        let (_, bugs) = evaluate_outcome_for_contract(&r, "Vault");
        // invariant_bounded appears in both body and summary — should only be counted once
        let count = bugs.iter().filter(|b| b.invariant_name == "invariant_bounded").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn zero_exit_is_pass() {
        let r = result(0, "Suite result: ok. 3 passed; 0 failed");
        let (outcome, bugs) = evaluate_outcome_for_contract(&r, "Vault");
        assert!(matches!(outcome, FuzzOutcome::Pass));
        assert!(bugs.is_empty());
    }

    #[test]
    fn non_invariant_failure_is_dev_test_failed() {
        let r = result(1, "[FAIL: assertion failed] testSetup() (gas: 1234)");
        let (outcome, bugs) = evaluate_outcome_for_contract(&r, "Vault");
        assert!(matches!(outcome, FuzzOutcome::DevTestFailed));
        assert!(bugs.is_empty());
    }
}
