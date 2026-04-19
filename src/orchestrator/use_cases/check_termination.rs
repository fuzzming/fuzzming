use crate::shared::{
    models::SessionState,
    responses::{fuzz_report::FuzzReport, termination_decision::TerminationDecision},
};

pub fn check_termination(report: &FuzzReport, state: &SessionState) -> TerminationDecision {
    todo!()
}
