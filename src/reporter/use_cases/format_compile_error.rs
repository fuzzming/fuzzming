use console::{Color, Style};

pub fn format_compile_error(contract_name: &str, round: u32, message: &str) -> String {
    let err_st  = Style::new().fg(Color::Red).bold();
    let label_st = Style::new().fg(Color::Color256(75)).bold();
    let muted   = Style::new().fg(Color::Color256(245));

    let header = format!(
        "\n  {}  {}  {}",
        err_st.apply_to("✗"),
        label_st.apply_to(contract_name),
        muted.apply_to(format!("compile error — round {}", round)),
    );

    let lines: String = message
        .lines()
        .take(20)
        .map(|l| format!("     {}", muted.apply_to(l)))
        .collect::<Vec<_>>()
        .join("\n");

    format!("{}\n{}\n", header, lines)
}
