use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_bug_report(outcome: &SessionOutcome) -> String {
    let bug_count = outcome.artifacts.call_sequences.len();

    let bug_blocks = if outcome.artifacts.call_sequences.is_empty() {
        "  (no call sequences captured)".to_string()
    } else {
        outcome
            .artifacts
            .call_sequences
            .iter()
            .enumerate()
            .map(|(i, seq)| format!("**Bug {}:**\n```\n{}\n```", i + 1, seq))
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        "## FuzzMing: {} bug(s) found in `{}` (round {})\n\n\
         {}\n\n\
         **Forge output:**\n```\n{}\n```",
        bug_count,
        outcome.contract_name,
        outcome.rounds_completed,
        bug_blocks,
        truncate(&outcome.artifacts.fuzz_output, 3000),
    )
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
