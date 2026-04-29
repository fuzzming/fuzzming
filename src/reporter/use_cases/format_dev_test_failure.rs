use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_dev_test_failure(outcome: &SessionOutcome) -> String {
    format!(
        "## FuzzMing: Forge Tests Failed for `{}` (round {})\n\n\
         **Output:**\n```\n{}\n```",
        outcome.contract_name,
        outcome.rounds_completed,
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
