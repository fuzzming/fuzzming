use crate::shared::models::RunnerResult;
use crate::shared::responses::fuzz_report::FuzzOutcome;

/// Inspect a forge test result and decide what happened.
///
/// Rules (applied in order):
/// 1. exit_code == 0                                          → Pass
/// 2. exit_code != 0, any `[FAIL` line mentions `invariant_` → Bug
/// 3. exit_code != 0, any `[FAIL` line found                 → DevTestFailed
/// 4. exit_code != 0, no `[FAIL` at all (compile error etc.) → DevTestFailed
///
/// Forge uses two formats — both are handled by checking if `invariant_` appears
/// anywhere on a failing line:
///   `[FAIL. Reason: …] invariant_balance() (runs: 100)`
///   `[FAIL: VaultInvariantTest::invariant_balance()] (runs: 256)`
pub fn evaluate_outcome(result: &RunnerResult) -> FuzzOutcome {
    if result.exit_code == 0 {
        return FuzzOutcome::Pass;
    }

    let combined = format!("{}\n{}", result.stdout, result.stderr);

    for line in combined.lines() {
        if line.contains("[FAIL") && line.contains("invariant_") {
            return FuzzOutcome::Bug;
        }
    }
    FuzzOutcome::DevTestFailed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(exit_code: i32, stdout: &str) -> RunnerResult {
        RunnerResult {
            exit_code,
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    #[test]
    fn pass_on_zero_exit() {
        let r = result(0, "Suite result: ok. 2 passed; 0 failed");
        assert!(matches!(evaluate_outcome(&r), FuzzOutcome::Pass));
    }

    #[test]
    fn bug_on_invariant_failure() {
        let stdout = "[FAIL. Reason: Invariant violation.] invariant_balance() (runs: 100, calls: 50, reverts: 2)";
        let r = result(1, stdout);
        assert!(matches!(evaluate_outcome(&r), FuzzOutcome::Bug));
    }

    #[test]
    fn bug_on_qualified_invariant_failure() {
        let stdout = "[FAIL: VaultInvariantTest::invariant_totalSupply()] (runs: 256)";
        let r = result(1, stdout);
        assert!(matches!(evaluate_outcome(&r), FuzzOutcome::Bug));
    }

    #[test]
    fn dev_test_failed_on_non_invariant_failure() {
        let stdout = "[FAIL. Reason: assertion failed] testSetup() (gas: 12345)";
        let r = result(1, stdout);
        assert!(matches!(evaluate_outcome(&r), FuzzOutcome::DevTestFailed));
    }

    #[test]
    fn dev_test_failed_on_compilation_error() {
        let r = result(1, "Error: Compiler run failed");
        assert!(matches!(evaluate_outcome(&r), FuzzOutcome::DevTestFailed));
    }

    #[test]
    fn dev_test_failed_on_setup_revert() {
        let stdout = "[FAIL. Reason: revert: setUp failed] setUp() (gas: 5000)";
        let r = result(1, stdout);
        assert!(matches!(evaluate_outcome(&r), FuzzOutcome::DevTestFailed));
    }

    #[test]
    fn bug_takes_priority_over_dev_failure_in_same_output() {
        // Both a dev test and an invariant fail — invariant is detected first → Bug
        let stdout = "[FAIL. Reason: revert] testSetup() (gas: 1)\n\
                      [FAIL. Reason: Invariant violation.] invariant_solvency() (runs: 10)";
        let r = result(1, stdout);
        assert!(matches!(evaluate_outcome(&r), FuzzOutcome::Bug));
    }
}
