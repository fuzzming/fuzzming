use crate::orchestrator::domain::Session;
use crate::shared::responses::fuzz_report::FuzzReport;
use crate::shared::responses::termination_decision::TerminationDecision;

pub fn check_termination(report: &FuzzReport, session: &Session) -> TerminationDecision {
    session.should_terminate(report)
}
