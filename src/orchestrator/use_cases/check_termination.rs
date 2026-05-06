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
        // Bug is no longer terminal — accumulate and continue hunting.
        FuzzOutcome::Bug => TerminationDecision { terminate: false, reason: None },
        // Compilation failure — let the LLM repair the code and retry, but stop if budget exhausted.
        FuzzOutcome::CompileError => {
            if state.rounds_remaining == 0 {
                TerminationDecision { terminate: true, reason: Some(TerminationReason::Exhausted) }
            } else {
                TerminationDecision { terminate: false, reason: None }
            }
        }
        FuzzOutcome::FullCoverage => TerminationDecision {
            terminate: true,
            reason: Some(TerminationReason::FullCoverage),
        },
        FuzzOutcome::DevTestFailed => TerminationDecision {
            terminate: true,
            reason: Some(TerminationReason::DevTestFailed),
        },
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
