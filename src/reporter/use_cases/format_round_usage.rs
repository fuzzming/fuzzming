use console::{Color, Style};

use crate::shared::responses::round_usage::RoundUsage;

pub fn format_round_usage(usage: &RoundUsage) -> String {
    let header_style = Style::new().fg(Color::Color256(63)).bold();
    let label_style = Style::new().fg(Color::Color256(75)).bold();
    let muted = Style::new().fg(Color::Color256(245));

    format!(
        "{} {}  {} {}  {} {}  {} {}",
        header_style.apply_to(format!("Round {}", usage.round)),
        muted.apply_to(&usage.contract_name),
        label_style.apply_to("tokens"),
        usage.usage.total_tokens,
        label_style.apply_to("prompt"),
        usage.usage.prompt_tokens,
        label_style.apply_to("completion"),
        usage.usage.completion_tokens,
    )
}
