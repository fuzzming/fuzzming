use crate::shared::models::ReportArtifacts;

pub fn format_bug_report(artifacts: &ReportArtifacts) -> String {
    let sequences = if artifacts.call_sequences.is_empty() {
        "  (no call sequence captured)".to_string()
    } else {
        artifacts
            .call_sequences
            .iter()
            .map(|s| format!("  {}", s))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "## FuzzMing: Bug Found in `{}` (round {})\n\n\
         **Failing call sequence:**\n```\n{}\n```\n\n\
         **Forge output:**\n```\n{}\n```",
        artifacts.contract_name,
        artifacts.round_history,
        sequences,
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
