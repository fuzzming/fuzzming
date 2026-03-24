use crate::interfaces::signals::{FuzzReport, TerminationDecision};
use crate::interfaces::state::SessionState;

pub fn check_termination(
    report: &FuzzReport,
    state: &SessionState,
) -> TerminationDecision {
    todo!()
}
