use crate::shared::responses::session_outcome::SessionOutcome;

pub fn format_exhausted_report(outcome: &SessionOutcome) -> String {
    let cov_section = format_coverage_snapshots(&outcome.coverage_snapshots);

    let bug_count = outcome.bugs.len();
    let summary = if bug_count == 0 {
        "no bugs found".to_string()
    } else {
        format!("{} bug{} found", bug_count, if bug_count == 1 { "" } else { "s" })
    };

    let bugs_section = if bug_count > 0 {
        // Group by invariant name, preserving first-seen order.
        let mut order: Vec<String> = Vec::new();
        let mut groups: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for b in &outcome.bugs {
            if !groups.contains_key(&b.invariant_name) {
                order.push(b.invariant_name.clone());
            }
            groups
                .entry(b.invariant_name.clone())
                .or_default()
                .push(b.call_sequence.clone());
        }

        let list = order
            .iter()
            .map(|name| {
                let seqs = &groups[name];
                let count = seqs.len();
                let label = if count > 1 {
                    format!("- `{}` ({} occurrences):", name, count)
                } else {
                    format!("- `{}`:", name)
                };
                let body = seqs
                    .iter()
                    .map(|seq| {
                        seq.lines()
                            .map(|line| format!("  {}", line))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("{}\n{}", label, body)
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!("\n\n**Bugs found:**\n{}", list)
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
