use crate::shared::models::{SessionConfig, SessionState};
use crate::shared::requests::session_request::SessionRequest;
use crate::shared::responses::fuzz_report::{FuzzOutcome, FuzzReport};
use crate::shared::responses::session_outcome::TerminationReason;
use crate::shared::responses::termination_decision::TerminationDecision;

pub struct Session {
    state: SessionState,
}

impl Session {
    pub fn new(request: &SessionRequest) -> Self {
        Self {
            state: SessionState {
                rounds_remaining: request.max_rounds,
                current_round: 0,
                config: request.config.clone(),
            },
        }
    }

    /// Evaluate whether the session should stop after a fuzz round.
    ///
    /// Terminates immediately on Bug, DevTestFailed, and FullCoverage.
    /// Terminates on Pass only when the round budget is exhausted.
    pub fn should_terminate(&self, report: &FuzzReport) -> TerminationDecision {
        match report.outcome {
            FuzzOutcome::Bug => TerminationDecision {
                terminate: true,
                reason: Some(TerminationReason::Bug),
                paths: Some(report.paths.clone()),
            },
            FuzzOutcome::DevTestFailed => TerminationDecision {
                terminate: true,
                reason: Some(TerminationReason::DevTestFailed),
                paths: Some(report.paths.clone()),
            },
            FuzzOutcome::FullCoverage => TerminationDecision {
                terminate: true,
                reason: Some(TerminationReason::FullCoverage),
                paths: Some(report.paths.clone()),
            },
            FuzzOutcome::Pass => {
                if self.state.rounds_remaining == 0 {
                    TerminationDecision {
                        terminate: true,
                        reason: Some(TerminationReason::Exhausted),
                        paths: Some(report.paths.clone()),
                    }
                } else {
                    TerminationDecision {
                        terminate: false,
                        reason: None,
                        paths: None,
                    }
                }
            }
        }
    }

    pub fn advance_round(&mut self) {
        self.state.current_round += 1;
        self.state.rounds_remaining = self.state.rounds_remaining.saturating_sub(1);
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn config(&self) -> &SessionConfig {
        &self.state.config
    }
}
