use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_exhausted_report(outcome: &SessionOutcome) -> String {
    let cov_section = format_coverage_snapshots(&outcome.coverage_snapshots);

    let bug_count = outcome.bugs.len(); // already deduplicated at accumulation
    let summary = if bug_count == 0 {
        "no bugs found".to_string()
    } else {
        format!(
            "{} bug{} found",
            bug_count,
            if bug_count == 1 { "" } else { "s" }
        )
    };

    let bugs_section = if bug_count > 0 {
        let list = outcome
            .bugs
            .iter()
            .map(|b| {
                let code_block = if b.invariant_code.is_empty() {
                    String::new()
                } else {
                    format!("\n```solidity\n{}\n```", b.invariant_code)
                };
                if b.call_sequence.is_empty() {
                    format!("- `{}`{}", b.invariant_name, code_block)
                } else {
                    let seq = b
                        .call_sequence
                        .lines()
                        .map(|line| format!("  {line}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("- `{}`{}\n{}", b.invariant_name, code_block, seq)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!("\n\n**Bugs found:**\n{list}")
    } else {
        String::new()
    };

    format!(
        "## FuzzMing: Rounds Exhausted for `{}` ({} rounds, {}){}\n\n{}",
        outcome.contract_name, outcome.rounds_completed, summary, bugs_section, cov_section,
    )
}

fn format_coverage_snapshots(snapshots: &[String]) -> String {
    if snapshots.is_empty() {
        return "**Coverage:** (no coverage data)".to_string();
    }
    snapshots
        .iter()
        .enumerate()
        .map(|(i, s)| format!("**Round {}:**\n```\n{}\n```", i + 1, s))
        .collect::<Vec<_>>()
        .join("\n\n")
}
