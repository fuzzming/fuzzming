use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_dev_test_failure(outcome: &SessionOutcome) -> String {
    format!(
        "## FuzzMing: Forge Tests Failed for `{}` (round {})",
        outcome.contract_name, outcome.rounds_completed,
    )
}
