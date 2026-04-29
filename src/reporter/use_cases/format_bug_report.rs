use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_bug_report(outcome: &SessionOutcome) -> String {
    let sequences = if outcome.artifacts.call_sequences.is_empty() {
        "  (no call sequence captured)".to_string()
    } else {
        outcome
            .artifacts
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
        outcome.contract_name,
        outcome.rounds_completed,
        sequences,
        truncate(&outcome.artifacts.fuzz_output, 3000),
    )
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
