use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::fuzzer::ports::outbound::TestRunnerPort;
use crate::shared::models::{BugInfo, CoverageResult, RunnerResult};

pub struct ForgeRunner {
    pub working_dir: PathBuf,
}

impl ForgeRunner {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }
}

fn forge_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let current = std::env::var("PATH").unwrap_or_default();
    format!("{home}/.foundry/bin:{current}")
}

fn extract_invariant_name(line: &str) -> Option<String> {
    let start = line.find("invariant_")?;
    let rest = &line[start..];
    let end = rest.find('(').unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

#[async_trait]
impl TestRunnerPort for ForgeRunner {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult> {
        let output = tokio::process::Command::new("forge")
            .args(["test"])
            .env("FOUNDRY_PROFILE", profile_name)
            .env("PATH", forge_path())
            .current_dir(&self.working_dir)
            .output()
            .await
            .context("failed to spawn `forge test`")?;

        Ok(RunnerResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    async fn run_coverage(&self, profile_name: &str) -> Result<CoverageResult> {
        let output = tokio::process::Command::new("forge")
            .args(["coverage", "--report", "lcov"])
            .env("FOUNDRY_PROFILE", profile_name)
            .env("PATH", forge_path())
            .current_dir(&self.working_dir)
            .output()
            .await
            .context("failed to spawn `forge coverage`")?;

        let exit_code = output.status.code().unwrap_or(-1);
        let lcov_content = tokio::fs::read_to_string(self.working_dir.join("lcov.info")).await.ok();

        Ok(CoverageResult { exit_code, lcov_content })
    }

    /// Parse all failing invariants for a contract out of forge stdout.
    ///
    /// Forge prints results twice (body + "Failing tests:" summary).
    /// We stop at the summary to avoid duplicates.
    fn collect_bugs(&self, stdout: &str, contract_name: &str) -> Vec<BugInfo> {
        let section_marker = format!("{}InvariantTest", contract_name);
        let mut in_section = false;
        let mut in_fail_block = false;
        let mut collecting_sequence = false;
        let mut sequence_lines: Vec<&str> = Vec::new();
        let mut bugs: Vec<BugInfo> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for line in stdout.lines() {
            if line.trim_start().starts_with("Failing tests:") {
                break;
            }

            if line.contains(&section_marker) {
                in_section = true;
                continue;
            }

            if in_section && line.contains("InvariantTest") && !line.contains(&section_marker) {
                break;
            }

            if !in_section {
                continue;
            }

            if line.trim_start().starts_with("[FAIL") {
                in_fail_block = true;
                collecting_sequence = false;
                sequence_lines.clear();
                continue;
            }

            if !in_fail_block {
                continue;
            }

            if line.contains("[Sequence]") || line.contains("[Shrunk sequence]") {
                collecting_sequence = true;
                continue;
            }

            if line.contains('╭') || line.contains('╰') || line.contains('├') {
                collecting_sequence = false;
                continue;
            }

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

            if collecting_sequence
                && !line.trim().is_empty()
                && !line.contains('│')
                && !line.contains('╰')
            {
                sequence_lines.push(line.trim());
            }
        }

        bugs
    }

    /// Capture the forge output section for one contract.
    fn filter_output(&self, stdout: &str, contract_name: &str) -> String {
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
    fn filter_lcov(&self, lcov: &str, contract_name: &str) -> String {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner() -> ForgeRunner {
        ForgeRunner::new(PathBuf::from("."))
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
        let r = runner();
        let bugs = r.collect_bugs(MULTI_BUG_OUTPUT, "Vault");
        assert_eq!(bugs.len(), 3);
        assert!(bugs.iter().any(|b| b.invariant_name == "invariant_bounded"));
        assert!(bugs.iter().any(|b| b.invariant_name == "invariant_in_range"));
        assert!(bugs.iter().any(|b| b.invariant_name == "invariant_never_zero"));
    }

    #[test]
    fn call_sequence_extracted_per_bug() {
        let r = runner();
        let bugs = r.collect_bugs(MULTI_BUG_OUTPUT, "Vault");
        let bounded = bugs.iter().find(|b| b.invariant_name == "invariant_bounded").unwrap();
        assert!(bounded.call_sequence.contains("handler_bigJump"));
        let never_zero =
            bugs.iter().find(|b| b.invariant_name == "invariant_never_zero").unwrap();
        assert!(never_zero.call_sequence.contains("handler_reset"));
    }

    #[test]
    fn no_duplicate_bugs_from_summary_section() {
        let r = runner();
        let bugs = r.collect_bugs(MULTI_BUG_OUTPUT, "Vault");
        let count = bugs.iter().filter(|b| b.invariant_name == "invariant_bounded").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn empty_bugs_for_non_invariant_failure() {
        let r = runner();
        let bugs = r.collect_bugs("[FAIL: assertion failed] testSetup() (gas: 1234)", "Vault");
        assert!(bugs.is_empty());
    }

    #[test]
    fn filter_output_captures_contract_section() {
        let r = runner();
        let out = r.filter_output(MULTI_BUG_OUTPUT, "Vault");
        assert!(out.contains("VaultInvariantTest"));
        assert!(out.contains("invariant_bounded"));
    }

    #[test]
    fn filter_lcov_keeps_matching_sf_records() {
        let lcov = "SF:src/Other.sol\nDA:1,1\nend_of_record\nSF:src/Vault.sol\nDA:5,1\nend_of_record\n";
        let r = runner();
        let out = r.filter_lcov(lcov, "Vault");
        assert!(out.contains("Vault.sol"));
        assert!(!out.contains("Other.sol"));
    }
}
