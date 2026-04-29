use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_exhausted_report(outcome: &SessionOutcome) -> String {
    let cov = outcome
        .artifacts
        .coverage_summary
        .as_deref()
        .unwrap_or("(no coverage data)");

    format!(
        "## FuzzMing: Rounds Exhausted for `{}` ({} rounds, no bugs found)\n\n\
         **Final coverage:**\n```\n{}\n```",
        outcome.contract_name, outcome.rounds_completed, cov,
    )
}
