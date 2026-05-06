use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_exhausted_report(outcome: &SessionOutcome) -> String {
    let cov = outcome
        .artifacts
        .coverage_summary
        .as_deref()
        .unwrap_or("(no coverage data)");

    let bug_count = outcome.bugs.len();
    let summary = if bug_count == 0 {
        "no bugs found".to_string()
    } else {
        format!("{} bug{} found", bug_count, if bug_count == 1 { "" } else { "s" })
    };

    let bugs_section = if bug_count > 0 {
        let list = outcome
            .bugs
            .iter()
            .map(|b| format!("- `{}`: {}", b.invariant_name, b.call_sequence))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n\n**Bugs found:**\n{}", list)
    } else {
        String::new()
    };

    format!(
        "## FuzzMing: Rounds Exhausted for `{}` ({} rounds, {}){}\n\n\
         **Final coverage:**\n```\n{}\n```",
        outcome.contract_name, outcome.rounds_completed, summary, bugs_section, cov,
    )
}
