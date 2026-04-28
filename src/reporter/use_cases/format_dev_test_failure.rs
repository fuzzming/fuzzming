use crate::shared::models::ReportArtifacts;

pub fn format_dev_test_failure(artifacts: &ReportArtifacts) -> String {
    format!(
        "## FuzzMing: Forge Tests Failed for `{}` (round {})\n\n\
         **Output:**\n```\n{}\n```",
        artifacts.contract_name,
        artifacts.round_history,
        truncate(&artifacts.fuzz_output, 3000),
    )
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
