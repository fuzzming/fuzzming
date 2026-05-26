use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_bug_report(outcome: &SessionOutcome) -> String {
    let bug_count = outcome.bugs.len();

    let bug_blocks = if outcome.bugs.is_empty() {
        "  (no call sequences captured)".to_string()
    } else {
        outcome
            .bugs
            .iter()
            .enumerate()
            .map(|(i, bug)| format!("**Bug {}:** {}\n```\n{}\n```", i + 1, bug.invariant_name, bug.call_sequence))
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        "## FuzzMing: {} bug(s) found in `{}` (round {})\n\n{}",
        bug_count,
        outcome.contract_name,
        outcome.rounds_completed,
        bug_blocks,
    )
}
