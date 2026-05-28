use std::collections::HashSet;

use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_bug_report(outcome: &SessionOutcome) -> String {
    let mut seen: HashSet<&str> = HashSet::new();
    let unique_bugs: Vec<_> = outcome
        .bugs
        .iter()
        .filter(|b| seen.insert(b.invariant_name.as_str()))
        .collect();

    let bug_count = unique_bugs.len();

    let bug_blocks = if unique_bugs.is_empty() {
        "  (no call sequences captured)".to_string()
    } else {
        unique_bugs
            .iter()
            .enumerate()
            .map(|(i, bug)| {
                let code_block = if bug.invariant_code.is_empty() {
                    String::new()
                } else {
                    format!("\n**Invariant:**\n```solidity\n{}\n```", bug.invariant_code)
                };
                if bug.call_sequence.is_empty() {
                    format!("**Bug {}:** {}{}", i + 1, bug.invariant_name, code_block)
                } else {
                    format!(
                        "**Bug {}:** {}{}\n**Call sequence:**\n```\n{}\n```",
                        i + 1,
                        bug.invariant_name,
                        code_block,
                        bug.call_sequence
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        "## FuzzMing: {} bug(s) found in `{}` (round {})\n\n{}",
        bug_count, outcome.contract_name, outcome.rounds_completed, bug_blocks,
    )
}
