use console::{Color, Style};

pub fn format_compile_error(round: u32, message: &str) -> String {
    let err_st  = Style::new().fg(Color::Red).bold();
    let muted   = Style::new().fg(Color::Color256(245));

    let header = format!(
        "\n  {}  {}",
        err_st.apply_to("✗"),
        muted.apply_to(format!("Solidity compile error — round {}", round)),
    );

    let lines: String = message
        .lines()
        .take(20)
        .map(|l| format!("     {}", muted.apply_to(l)))
        .collect::<Vec<_>>()
        .join("\n");

    format!("{}\n{}\n", header, lines)
}
