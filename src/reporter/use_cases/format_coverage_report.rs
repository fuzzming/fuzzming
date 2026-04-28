use crate::shared::models::ReportArtifacts;

pub fn format_coverage_report(artifacts: &ReportArtifacts) -> String {
    let cov = artifacts
        .coverage_summary
        .as_deref()
        .unwrap_or("(no coverage data)");

    format!(
        "## FuzzMing: Full Coverage Achieved for `{}` (round {})\n\n\
         **Coverage summary:**\n```\n{}\n```",
        artifacts.contract_name, artifacts.round_history, cov,
    )
}
