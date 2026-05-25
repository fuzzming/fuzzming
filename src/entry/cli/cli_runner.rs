use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use console::{Color, Style};
use tracing_subscriber::EnvFilter;

use crate::composition::composition_root::CompositionRoot;
use crate::entry::cli::arg_parser::{parse_args, Command};
use crate::entry::cli::interactive::resolve_cli_config;
use crate::entry::cli::ui::CliUi;
use crate::shared::models::{Fuzzer, Language, SessionConfig};
use crate::shared::requests::session_request::SessionRequest;
use crate::shared::responses::session_outcome::{SessionOutcome, TerminationReason};

pub struct CliRunner;

impl CliRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) -> Result<()> {
        let args = parse_args();
        init_tracing(args.verbose);
        let ui = CliUi::new();

        match &args.command {
            Some(Command::Guide) => {
                print_extended_help(&ui);
                return Ok(());
            }
            Some(Command::Report { workspace_root }) => {
                return handle_report(workspace_root.clone(), &ui);
            }
            Some(Command::Config { reset }) => {
                return handle_config(*reset, &ui);
            }
            None => {}
        }

        let resolved = resolve_cli_config(&args)?;

        let config = SessionConfig {
            model: resolved.model.clone(),
            llm_key: resolved.llm_key.clone(),
            language: Language::Solidity,
            fuzzer: Fuzzer::Foundry,
            workspace_root: resolved.workspace_root.clone(),
            max_tokens: resolved.max_tokens,
            llm_timeout_secs: resolved.llm_timeout_secs,
            full_coverage_rounds: resolved.full_coverage_rounds,
        };
        let request = SessionRequest {
            target_paths: resolved.targets.clone(),
            max_rounds: resolved.max_rounds,
            config: config.clone(),
        };
        let orchestrator = CompositionRoot::build(config);

        let outcomes = match orchestrator.run(request).await {
            Ok(outcomes) => outcomes,
            Err(err) => {
                let message = err.to_string();
                ui.error("FuzzMing stopped early.");
                if message.contains("litellm")
                    || message.contains("LLM")
                    || message.contains("executor")
                {
                    ui.warn("Hint: try a stronger model or check your LLM provider key.");
                }
                std::process::exit(1);
            }
        };

        let has_bugs = outcomes.iter().any(|o| {
            matches!(o.reason, TerminationReason::Bug | TerminationReason::DevTestFailed)
                || !o.artifacts.call_sequences.is_empty()
        });

        if has_bugs {
            std::process::exit(1);
        }

        Ok(())
    }
}

// ── subcommand: report ────────────────────────────────────────────────────────

fn handle_report(workspace_root: Option<PathBuf>, ui: &CliUi) -> Result<()> {
    let root = workspace_root.unwrap_or_else(|| PathBuf::from("."));
    let fuzzming_dir = root.join(".fuzzming");

    let header_st = Style::new().fg(Color::Color256(99)).bold();
    let label_st = Style::new().fg(Color::Color256(75)).bold();
    let muted = Style::new().fg(Color::Color256(245));
    let ok_st = Style::new().fg(Color::Green).bold();
    let err_st = Style::new().fg(Color::Red).bold();

    if !fuzzming_dir.exists() {
        ui.warn("No .fuzzming/ directory found. Run fuzzming first to generate reports.");
        return Ok(());
    }

    println!();
    println!("{}", header_st.apply_to("  ◆ FuzzMing — Previous Run Reports"));
    println!("{}", muted.apply_to("  ──────────────────────────────────────────"));
    println!();

    let mut found = false;
    let mut entries: Vec<_> = fs::read_dir(&fuzzming_dir)?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let contract = entry.file_name().to_string_lossy().to_string();
        let fuzz_out = entry.path().join("fuzz_output.txt");
        let lcov = entry.path().join("lcov.info");

        if !fuzz_out.exists() {
            continue;
        }
        found = true;

        // Contract header
        println!(
            "  {}  {}",
            ok_st.apply_to("◆"),
            label_st.apply_to(&contract)
        );

        // Coverage summary from lcov if present
        if lcov.exists() {
            let content = fs::read_to_string(&lcov).unwrap_or_default();
            let (lf, lh) = parse_lcov_totals(&content);
            if lf > 0 {
                let pct = (lh as f64 / lf as f64 * 100.0) as u32;
                println!(
                    "     {}  {}/{} lines covered  ({}%)",
                    muted.apply_to("coverage:"),
                    lh,
                    lf,
                    if pct >= 80 {
                        ok_st.apply_to(pct.to_string()).to_string()
                    } else {
                        err_st.apply_to(pct.to_string()).to_string()
                    }
                );
            }
        }

        // Outcome: termination reason and bugs from outcome.json
        let outcome_path = entry.path().join("outcome.json");
        if outcome_path.exists() {
            if let Ok(json) = fs::read_to_string(&outcome_path) {
                if let Ok(outcome) = serde_json::from_str::<SessionOutcome>(&json) {
                    let reason_str = match outcome.reason {
                        TerminationReason::Bug => "Bug found",
                        TerminationReason::Exhausted => "Rounds exhausted",
                        TerminationReason::FullCoverage => "Full coverage",
                        TerminationReason::DevTestFailed => "Test setup failed",
                    };
                    println!(
                        "     {}  {}  ({} rounds)",
                        muted.apply_to("result:"),
                        if matches!(outcome.reason, TerminationReason::Bug | TerminationReason::DevTestFailed) {
                            err_st.apply_to(reason_str).to_string()
                        } else {
                            ok_st.apply_to(reason_str).to_string()
                        },
                        outcome.rounds_completed
                    );
                    for bug in &outcome.bugs {
                        println!("     {}  {}", err_st.apply_to("bug:"), label_st.apply_to(&bug.invariant_name));
                        for line in bug.call_sequence.lines().take(4) {
                            println!("       {}", muted.apply_to(line));
                        }
                    }
                }
            }
        }

        // Last few lines of fuzz output
        let content = fs::read_to_string(&fuzz_out).unwrap_or_default();
        let tail: Vec<&str> = content.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect();
        for line in tail {
            if !line.trim().is_empty() {
                println!("     {}", muted.apply_to(line));
            }
        }
        println!();
    }

    if !found {
        ui.warn("No contract reports found in .fuzzming/. Run fuzzming first.");
    }

    Ok(())
}

fn parse_lcov_totals(lcov: &str) -> (u64, u64) {
    let mut lf = 0u64;
    let mut lh = 0u64;
    for line in lcov.lines() {
        if let Some(v) = line.strip_prefix("LF:") {
            lf += v.trim().parse::<u64>().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("LH:") {
            lh += v.trim().parse::<u64>().unwrap_or(0);
        }
    }
    (lf, lh)
}

// ── subcommand: config ────────────────────────────────────────────────────────

fn handle_config(reset: bool, ui: &CliUi) -> Result<()> {
    let config_path = std::env::current_dir()?.join("fuzzming.config.txt");
    let header_st = Style::new().fg(Color::Color256(99)).bold();
    let label_st = Style::new().fg(Color::Color256(75)).bold();
    let muted = Style::new().fg(Color::Color256(245));

    if reset {
        if config_path.exists() {
            fs::remove_file(&config_path)?;
            ui.success("✓ Config file removed. The next run will re-prompt for settings.");
        } else {
            ui.warn("No fuzzming.config.txt found — nothing to reset.");
        }
        return Ok(());
    }

    if !config_path.exists() {
        ui.warn("No fuzzming.config.txt found. Run fuzzming --interactive to create one.");
        return Ok(());
    }

    println!();
    println!("{}", header_st.apply_to("  ◆ FuzzMing — Saved Configuration"));
    println!("{}", muted.apply_to("  ──────────────────────────────────────────"));
    println!();

    let content = fs::read_to_string(&config_path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with("llm_key=") {
            // Mask the key value for security
            println!("  {}  {}", label_st.apply_to("llm_key"), muted.apply_to("****"));
        } else if let Some((k, v)) = trimmed.split_once('=') {
            println!("  {}  {}", label_st.apply_to(k), muted.apply_to(v));
        }
    }

    println!();
    println!(
        "  {}",
        muted.apply_to("Use `fuzzming config --reset` to clear and re-prompt on next run.")
    );
    println!();

    Ok(())
}

// ── guide ─────────────────────────────────────────────────────────────────────

fn print_extended_help(ui: &CliUi) {
    let header = console::Style::new().fg(console::Color::Color256(99)).bold();
    let label  = console::Style::new().fg(console::Color::Color256(75)).bold();
    let dim    = console::Style::new().fg(console::Color::Color256(245));
    let hi     = console::Style::new().fg(console::Color::Color256(117));   // cyan-ish for inline code

    ui.banner();

    // ── Overview ──────────────────────────────────────────────────────────────
    println!("{}", header.apply_to("  FUZZMING — AI-powered Solidity smart contract fuzzer"));
    println!("{}", dim.apply_to("  Point it at a Foundry project. Watch it think. Let it find bugs."));
    println!();
    println!("{}", dim.apply_to("  Usage:  fuzzming [FLAGS]"));
    println!("{}", dim.apply_to("          fuzzming <SUBCOMMAND> [FLAGS]"));
    println!();

    // ── Subcommands ───────────────────────────────────────────────────────────
    println!("{}", header.apply_to("  SUBCOMMANDS"));
    println!("{}", dim.apply_to("  ──────────────────────────────────────────────────────────────────"));
    println!();

    // guide
    println!("  {}  {}",
        label.apply_to("guide"),
        dim.apply_to("Show the full CLI reference and examples in the terminal"));
    println!("  {}", dim.apply_to("  No flags."));
    println!();
    println!("  {}", dim.apply_to("  Example:"));
    println!("    {}", hi.apply_to("fuzzming guide"));
    println!();

    // report
    println!("  {}  {}",
        label.apply_to("report"),
        dim.apply_to("Print a summary report from a previous run"));
    println!("  {}", dim.apply_to("  Reads .fuzzming/<Contract>/ artifacts — coverage %, last fuzz output."));
    println!();
    println!("  {}",  dim.apply_to("  Flags:"));
    println!("    {}  {}",
        label.apply_to("--workspace-root <DIR>"),
        dim.apply_to("Foundry project root that was fuzzed (default: \".\")"));
    println!();
    println!("  {}", dim.apply_to("  Examples:"));
    println!("    {}", hi.apply_to("fuzzming report"));
    println!("    {}", hi.apply_to("fuzzming report --workspace-root ./my-foundry-project"));
    println!();

    // config
    println!("  {}  {}",
        label.apply_to("config"),
        dim.apply_to("View or reset the saved fuzzming.config.txt"));
    println!("  {}", dim.apply_to("  Without flags: prints all saved keys (API key is always masked)."));
    println!("  {}", dim.apply_to("  With --reset:  deletes the config so the next run re-prompts."));
    println!();
    println!("  {}",  dim.apply_to("  Flags:"));
    println!("    {}  {}",
        label.apply_to("--reset"),
        dim.apply_to("Delete fuzzming.config.txt — next run will re-prompt for all settings"));
    println!();
    println!("  {}", dim.apply_to("  Examples:"));
    println!("    {}", hi.apply_to("fuzzming config"));
    println!("    {}", hi.apply_to("fuzzming config --reset"));
    println!();

    // ── Flags (main command) ──────────────────────────────────────────────────
    println!("{}", header.apply_to("  FLAGS  (fuzzming [FLAGS])"));
    println!("{}", dim.apply_to("  ──────────────────────────────────────────────────────────────────"));
    println!();
    let flags: &[(&str, &str, &str)] = &[
        ("--targets <PATHS...>",   "",          "Paths to target Solidity contracts"),
        ("--max-rounds <N>",       "",          "Maximum number of fuzzing rounds (default: 10)"),
        ("--model <ID>",           "LLM_MODEL", "LLM model identifier"),
        ("--llm-key <KEY>",        "LLM_KEY",   "LLM API key"),
        ("--workspace-root <DIR>", "",          "Foundry project root (default: \".\")"),
        ("--interactive",          "",          "Force interactive config prompts even when a config file exists"),
        ("--ci-mode",              "",          "Structured output for CI/CD pipelines (exit 1 on bugs)"),
        ("--verbose",              "",          "Enable verbose trace logs"),
    ];
    for (flag, env, desc) in flags {
        if env.is_empty() {
            println!("  {}  {}", label.apply_to(*flag), dim.apply_to(*desc));
        } else {
            println!("  {}  {}  {}",
                label.apply_to(*flag),
                dim.apply_to(*desc),
                dim.apply_to(&format!("[env: {}]", env)));
        }
    }
    println!();

    // ── Examples ─────────────────────────────────────────────────────────────
    println!("{}", header.apply_to("  EXAMPLES"));
    println!("{}", dim.apply_to("  ──────────────────────────────────────────────────────────────────"));
    println!();
    let examples: &[(&str, &str)] = &[
        ("fuzzming",
         "Interactive first run — prompts for workspace, model, key, etc."),
        ("fuzzming --interactive",
         "Force prompts even when a saved config already exists"),
        ("fuzzming --workspace-root ./project --targets src/Vault.sol",
         "Non-interactive run against a specific contract"),
        ("fuzzming --model groq/llama-3.3-70b-versatile --llm-key $LLM_KEY",
         "Pass model and key directly (skips interactive prompts)"),
        ("fuzzming --ci-mode --llm-key $LLM_KEY",
         "CI mode — exits 1 if bugs found, 0 if clean"),
        ("fuzzming guide",
         "Show this reference"),
        ("fuzzming report",
         "Show report from the last run in the current directory"),
        ("fuzzming report --workspace-root ./my-foundry-project",
         "Show report from a run in a different directory"),
        ("fuzzming config",
         "View saved settings (key is masked)"),
        ("fuzzming config --reset",
         "Delete config — next run will re-prompt"),
    ];
    for (cmd, desc) in examples {
        println!("  {}", hi.apply_to(*cmd));
        println!("    {}", dim.apply_to(*desc));
        println!();
    }
}

fn init_tracing(verbose: bool) {
    let filter = if verbose {
        EnvFilter::from_default_env()
    } else {
        EnvFilter::new("error")
    };

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

impl Default for CliRunner {
    fn default() -> Self {
        Self::new()
    }
}
