use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_coverage_report(outcome: &SessionOutcome) -> String {
    let cov = outcome
        .artifacts
        .coverage_summary
        .as_deref()
        .unwrap_or("(no coverage data)");

    format!(
        "## FuzzMing: Full Coverage Achieved for `{}` ({} rounds)\n\n\
         **Final coverage:**\n```\n{}\n```",
        outcome.contract_name, outcome.rounds_completed, cov,
    )
}
