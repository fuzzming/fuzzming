use crate::shared::{
    models::SessionState,
    responses::{
        fuzz_report::{FuzzOutcome, FuzzReport},
        session_outcome::TerminationReason,
        termination_decision::TerminationDecision,
    },
};

pub fn check_termination(report: &FuzzReport, state: &SessionState) -> TerminationDecision {
    match report.outcome {
        FuzzOutcome::Bug => {
            if state.rounds_remaining == 0 {
                TerminationDecision { terminate: true, reason: Some(TerminationReason::Exhausted) }
            } else {
                TerminationDecision { terminate: false, reason: None }
            }
        }
        // Compilation failure — let the LLM repair the code and retry, but stop if budget exhausted.
        FuzzOutcome::CompileError => {
            if state.rounds_remaining == 0 {
                TerminationDecision { terminate: true, reason: Some(TerminationReason::CompileError) }
            } else {
                TerminationDecision { terminate: false, reason: None }
            }
        }
        FuzzOutcome::FullCoverage => TerminationDecision {
            terminate: true,
            reason: Some(TerminationReason::FullCoverage),
        },
        // DevTestFailed (setUp revert, assertion in test logic, etc.) is just as fixable as
        // CompileError — feed the output back to the LLM and retry until rounds are exhausted.
        FuzzOutcome::DevTestFailed => {
            if state.rounds_remaining == 0 {
                TerminationDecision { terminate: true, reason: Some(TerminationReason::DevTestFailed) }
            } else {
                TerminationDecision { terminate: false, reason: None }
            }
        }
        FuzzOutcome::Pass => {
            if state.rounds_remaining == 0 {
                TerminationDecision {
                    terminate: true,
                    reason: Some(TerminationReason::Exhausted),
                }
            } else {
                TerminationDecision { terminate: false, reason: None }
            }
        }
    }
}
