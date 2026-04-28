use crate::shared::models::ReportArtifacts;

pub fn format_exhausted_report(artifacts: &ReportArtifacts) -> String {
    let cov = artifacts
        .coverage_summary
        .as_deref()
        .unwrap_or("(no coverage data)");

    format!(
        "## FuzzMing: Rounds Exhausted for `{}` ({} rounds, no bugs found)\n\n\
         **Final coverage:**\n```\n{}\n```",
        artifacts.contract_name, artifacts.round_history, cov,
    )
}
