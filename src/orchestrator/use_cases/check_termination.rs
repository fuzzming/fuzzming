use crate::shared::{
    models::SessionState,
    responses::{
        fuzz_report::{FuzzOutcome, FuzzReport},
        termination_decision::TerminationDecision,
    },
};
use crate::shared::responses::session_outcome::TerminationReason;

pub fn check_termination(report: &FuzzReport, state: &SessionState) -> TerminationDecision {
    match report.outcome {
        FuzzOutcome::Bug => TerminationDecision {
            terminate: true,
            reason: Some(TerminationReason::Bug),
        },
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
