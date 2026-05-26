use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_coverage_report(outcome: &SessionOutcome) -> String {
    let cov_section = format_coverage_snapshots(&outcome.coverage_snapshots);

    format!(
        "## FuzzMing: Full Coverage Achieved for `{}` ({} rounds)\n\n{}",
        outcome.contract_name, outcome.rounds_completed, cov_section,
    )
}

fn format_coverage_snapshots(snapshots: &[String]) -> String {
    if snapshots.is_empty() {
        return "**Coverage:** (no coverage data)".to_string();
    }
    snapshots
        .iter()
        .enumerate()
        .map(|(i, s)| format!("**Round {}:**\n```\n{}\n```", i + 1, s))
        .collect::<Vec<_>>()
        .join("\n\n")
}
