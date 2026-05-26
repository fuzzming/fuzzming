use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use console::{Color, Style};
use tracing_subscriber::EnvFilter;

use crate::composition::composition_root::CompositionRoot;
use crate::demo::DemoCompositionRoot;
use crate::entry::cli::arg_parser::{parse_args, Command, RunArgs};
use crate::entry::cli::interactive::{resolve_cli_config, workspace_root_from_config};
use crate::entry::cli::ui::CliUi;
use crate::reporter::use_cases::{
    format_bug_report, format_compile_error_outcome, format_coverage_report,
    format_dev_test_failure, format_exhausted_report,
};
use crate::shared::models::{Fuzzer, Language, SessionConfig};
use crate::shared::requests::session_request::SessionRequest;
use crate::shared::responses::session_outcome::{SessionOutcome, TerminationReason};

pub struct CliRunner;

impl CliRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) -> Result<()> {
        let ui = CliUi::new();
        ui.banner();
        let args = match parse_args() {
            Ok(a) => a,
            Err(e) => {
                let msg = e.to_string();
                // Extract the first meaningful line (skip blank lines and "error:" prefix)
                let first = msg.lines()
                    .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with("Usage") && !l.trim_start().starts_with("For more"))
                    .unwrap_or("invalid command")
                    .trim_start_matches("error: ")
                    .trim();
                ui.error(first);
                println!();
                ui.warn("Run 'fuzzming --help' to see available subcommands.");
                std::process::exit(1);
            }
        };

        if args.help {
            print_extended_help(&ui);
            return Ok(());
        }

        match args.command {
            Some(Command::Guide) => {
                print_extended_help(&ui);
                return Ok(());
            }
            Some(Command::Report { workspace_root }) => {
                return handle_report(workspace_root, &ui);
            }
            Some(Command::Config { reset }) => {
                return handle_config(reset, &ui);
            }
            Some(Command::Run(run_args)) => {
                return handle_run(run_args, &ui).await;
            }
            None => {
                ui.error("a subcommand is required");
                println!();
                print_extended_help(&ui);
                std::process::exit(1);
            }
        }
    }
}

async fn handle_run(args: RunArgs, ui: &CliUi) -> Result<()> {
    init_tracing(args.verbose);
    if args.demo {
        return run_demo().await;
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
        prompt_mode: resolved.prompt_mode.clone(),
    };
    let request = SessionRequest {
        target_paths: resolved.targets.clone(),
        max_rounds: resolved.max_rounds,
        config: config.clone(),
    };
    let orchestrator = CompositionRoot::build(config);

    let tokens_label = match resolved.max_tokens {
        Some(n) => n.to_string(),
        None => "unlimited".to_string(),
    };
    ui.info(&format!(
        "model: {}  |  max rounds: {}  |  max tokens/call: {}",
        resolved.model, resolved.max_rounds, tokens_label
    ));

    let outcomes = match orchestrator.run(request).await {
        Ok(outcomes) => outcomes,
        Err(err) => {
            let message = format!("{:#}", err);
            ui.error(&format!("FuzzMing stopped early: {}", message));
            if message.contains("timed out") {
                ui.warn("Hint: increase --llm-timeout-secs or try a faster model.");
            } else if message.contains("litellm") || message.contains("completion failed") {
                ui.warn("Hint: check your --llm-key and --model are correct.");
            } else if message.contains("forge") || message.contains("PATH") {
                ui.warn("Hint: make sure Foundry is installed and `forge` is on your PATH.");
            } else if message.contains("LLM patch failed") {
                ui.warn("Hint: the LLM returned a malformed edit, run again or try a stronger model.");
            }
            std::process::exit(1);
        }
    };

    print_outcome_reports(&outcomes);
    print_aggregate_summary(&outcomes);

    let has_bugs = outcomes.iter().any(|o| {
        matches!(o.reason, TerminationReason::Bug | TerminationReason::DevTestFailed | TerminationReason::CompileError)
            || !o.bugs.is_empty()
    });

    if has_bugs {
        std::process::exit(1);
    }

    Ok(())
}

// ── per-contract outcome reports ──────────────────────────────────────────────

fn print_outcome_reports(outcomes: &[SessionOutcome]) {
    println!();
    for outcome in outcomes {
        let msg = match outcome.reason {
            TerminationReason::Bug => format_bug_report(outcome),
            TerminationReason::FullCoverage => format_coverage_report(outcome),
            TerminationReason::DevTestFailed => format_dev_test_failure(outcome),
            TerminationReason::Exhausted => format_exhausted_report(outcome),
            TerminationReason::CompileError => format_compile_error_outcome(outcome),
        };
        println!("{}", msg);
        println!();
    }
}

// ── aggregate summary ─────────────────────────────────────────────────────────

fn print_aggregate_summary(outcomes: &[SessionOutcome]) {
    if outcomes.is_empty() {
        return;
    }

    let header_st  = Style::new().fg(Color::Color256(99)).bold();
    let label_st   = Style::new().fg(Color::Color256(75)).bold();
    let muted      = Style::new().fg(Color::Color256(245));
    let ok_st      = Style::new().fg(Color::Green).bold();
    let err_st     = Style::new().fg(Color::Red).bold();
    let warn_st    = Style::new().fg(Color::Color256(208)).bold();

    let total: usize = outcomes.len();

    // Contracts where the fuzzer ran and found no bugs.
    let passed: usize = outcomes.iter().filter(|o| {
        o.bugs.is_empty()
            && matches!(o.reason, TerminationReason::Exhausted | TerminationReason::FullCoverage)
    }).count();

    // Contracts with actual invariant violations.
    let with_bugs: usize = outcomes.iter().filter(|o| {
        !o.bugs.is_empty() || matches!(o.reason, TerminationReason::Bug)
    }).count();

    // Contracts whose test code never ran (compile error or forge test setup failure).
    let not_tested: usize = outcomes.iter().filter(|o| {
        matches!(o.reason, TerminationReason::CompileError | TerminationReason::DevTestFailed)
    }).count();

    // Subsets of not_tested.
    let compile_errors: usize = outcomes.iter().filter(|o| {
        matches!(o.reason, TerminationReason::CompileError)
    }).count();
    let setup_failed: usize = outcomes.iter().filter(|o| {
        matches!(o.reason, TerminationReason::DevTestFailed)
    }).count();

    let total_rounds: u32 = outcomes.iter().map(|o| o.rounds_completed).sum();
    let total_bugs: usize = outcomes.iter().map(|o| o.bugs.len()).sum();

    println!();
    println!("{}", header_st.apply_to("  ◆ FuzzMing: Session Summary"));
    println!("{}", muted.apply_to("  ──────────────────────────────────────────"));
    println!("  {}    {}", label_st.apply_to("contracts:"), muted.apply_to(total.to_string()));
    println!("  {}       {}",
        label_st.apply_to("passed:"),
        if passed == total { ok_st.apply_to(passed.to_string()).to_string() }
        else { muted.apply_to(passed.to_string()).to_string() }
    );
    println!("  {}    {}",
        label_st.apply_to("with bugs:"),
        if with_bugs > 0 { err_st.apply_to(with_bugs.to_string()).to_string() }
        else { ok_st.apply_to("0".to_string()).to_string() }
    );
    println!("  {}   {}",
        label_st.apply_to("not tested:"),
        if not_tested > 0 { warn_st.apply_to(not_tested.to_string()).to_string() }
        else { ok_st.apply_to("0".to_string()).to_string() }
    );
    println!("  {}  {}  {}",
        label_st.apply_to("  compile errors:"),
        if compile_errors > 0 { warn_st.apply_to(compile_errors.to_string()).to_string() }
        else { muted.apply_to("0".to_string()).to_string() },
        muted.apply_to("(test code never compiled)"),
    );
    println!("  {}   {}  {}",
        label_st.apply_to("  setup failed:"),
        if setup_failed > 0 { warn_st.apply_to(setup_failed.to_string()).to_string() }
        else { muted.apply_to("0".to_string()).to_string() },
        muted.apply_to("(compiled, but setUp/forge failed)"),
    );
    println!("  {}      {}", label_st.apply_to("rounds:"), muted.apply_to(total_rounds.to_string()));
    println!("  {}        {}",
        label_st.apply_to("bugs:"),
        if total_bugs > 0 { err_st.apply_to(total_bugs.to_string()).to_string() }
        else { ok_st.apply_to("0".to_string()).to_string() }
    );
    println!();
}

// ── subcommand: report ────────────────────────────────────────────────────────

fn handle_report(workspace_root: Option<PathBuf>, ui: &CliUi) -> Result<()> {
    let root = workspace_root
        .or_else(workspace_root_from_config)
        .unwrap_or_else(|| PathBuf::from("."));
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
    println!("{}", header_st.apply_to("  ◆ FuzzMing: Previous Run Reports"));
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

        // Outcome: termination reason and bugs from outcome.json
        let outcome_path = entry.path().join("outcome.json");
        if outcome_path.exists() {
            if let Ok(json) = fs::read_to_string(&outcome_path) {
                if let Ok(outcome) = serde_json::from_str::<SessionOutcome>(&json) {
                    let has_bugs = !outcome.bugs.is_empty()
                        || matches!(outcome.reason, TerminationReason::Bug | TerminationReason::DevTestFailed | TerminationReason::CompileError);

                    // Only show coverage when the run was clean — when bugs are found,
                    // forge coverage was never run for that round so lcov.info is stale.
                    if !has_bugs && lcov.exists() {
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

                    let reason_str = match outcome.reason {
                        TerminationReason::Bug => "Bug found",
                        TerminationReason::Exhausted => "Rounds exhausted",
                        TerminationReason::FullCoverage => "Full coverage",
                        TerminationReason::DevTestFailed => "Test setup failed",
                        TerminationReason::CompileError => "Compile error (never ran)",
                    };
                    println!(
                        "     {}  {}  ({} rounds)",
                        muted.apply_to("result:"),
                        if matches!(outcome.reason, TerminationReason::Bug | TerminationReason::DevTestFailed | TerminationReason::CompileError) {
                            err_st.apply_to(reason_str).to_string()
                        } else {
                            ok_st.apply_to(reason_str).to_string()
                        },
                        outcome.rounds_completed
                    );
                    for bug in &outcome.bugs {
                        println!("     {}  {}", err_st.apply_to("bug:"), label_st.apply_to(&bug.invariant_name));
                        for line in bug.call_sequence.lines() {
                            println!("       {}", muted.apply_to(line));
                        }
                    }
                }
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
    let config_path = std::env::current_dir()?.join("fuzzming.config");
    let header_st = Style::new().fg(Color::Color256(99)).bold();
    let label_st = Style::new().fg(Color::Color256(75)).bold();
    let muted = Style::new().fg(Color::Color256(245));

    if reset {
        if config_path.exists() {
            fs::remove_file(&config_path)?;
            ui.success("✓ Config file removed. The next run will re-prompt for settings.");
        } else {
            ui.warn("No fuzzming.config found, nothing to reset.");
        }
        return Ok(());
    }

    if !config_path.exists() {
        ui.warn("No fuzzming.config found. Run fuzzming --interactive to create one.");
        return Ok(());
    }

    println!();
    println!("{}", header_st.apply_to("  ◆ FuzzMing: Saved Configuration"));
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

fn print_extended_help(_ui: &CliUi) {
    let header = console::Style::new().fg(console::Color::Color256(99)).bold();
    let label  = console::Style::new().fg(console::Color::Color256(75)).bold();
    let dim    = console::Style::new().fg(console::Color::Color256(245));
    let hi     = console::Style::new().fg(console::Color::Color256(117));
    let sep    = console::Style::new().fg(console::Color::Color256(238));

    // Render a flag+hint string padded to a fixed column width, then the description.
    let print_flag_row = |indent: &str, flag: &str, hint: &str, desc: &str, col: usize| {
        let flag_str = if hint.is_empty() {
            flag.to_string()
        } else {
            format!("{} {}", flag, hint)
        };
        println!("{}{}   {}",
            indent,
            label.apply_to(format!("{:<col$}", flag_str)),
            dim.apply_to(desc),
        );
    };

    struct CommandDoc {
        name:        &'static str,
        one_liner:   &'static str,
        description: &'static [&'static str],
        flags:       &'static [(&'static str, &'static str, &'static str)],
        examples:    &'static [(&'static str, &'static str)],
    }

    let commands: &[CommandDoc] = &[
        CommandDoc {
            name:      "run",
            one_liner: "Start a fuzzing session against one or more contracts",
            description: &[
                "Loads fuzzming.config if present, then prompts for any missing values.",
                "Use --defaults or --from-config to skip all prompts entirely.",
            ],
            flags: &[
                ("--targets",        "<PATHS...>", "Paths to target Solidity contracts"),
                ("--max-rounds",     "<N>",        "Maximum fuzzing rounds (default: 10)"),
                ("--model",          "<ID>",       "LLM model identifier  [env: LLM_MODEL]"),
                ("--llm-key",        "<KEY>",      "LLM API key  [env: LLM_KEY]"),
                ("--workspace-root", "<DIR>",      "Foundry project root (default: \".\")"),
                ("--max-tokens",     "<N>",        "Max tokens per LLM call (omit for no limit)"),
                ("--defaults",       "",           "Skip all prompts, use defaults + flags/env vars"),
                ("--from-config",    "",           "Skip all prompts, read everything from fuzzming.config"),
                ("--interactive",    "",           "Force interactive prompts even when config exists"),
                ("--demo",           "",           "Run with mock adapters, no LLM calls, no tokens spent"),
                ("--verbose",        "",           "Enable verbose trace logs"),
            ],
            examples: &[
                ("fuzzming run",                                                           "Interactive: prompts for all missing values"),
                ("fuzzming run --targets src/Vault.sol --max-rounds 5",                   "Non-interactive with explicit flags"),
                ("fuzzming run --defaults --targets src/Vault.sol",                       "No prompts, defaults + flags/env vars"),
                ("fuzzming run --from-config",                                             "No prompts, read everything from fuzzming.config"),
                ("fuzzming run --interactive",                                             "Force prompts even when config exists"),
                ("fuzzming run --demo",                                                    "Mock run, no LLM calls"),
            ],
        },
        CommandDoc {
            name:      "guide",
            one_liner: "Show the full CLI reference and examples in the terminal",
            description: &[
                "Prints a structured guide to stdout.",
                "Useful as a quick reference without leaving the terminal.",
            ],
            flags:    &[],
            examples: &[
                ("fuzzming guide", "Print this reference"),
            ],
        },
        CommandDoc {
            name:      "report",
            one_liner: "Print a summary report from a previous run",
            description: &[
                "Reads .fuzzming/<Contract>/ artifacts written during the last session.",
                "Shows per-contract coverage % and the tail of forge fuzz output.",
            ],
            flags: &[
                ("--workspace-root", "<DIR>", "Foundry project root to read artifacts from (default: \".\")"),
            ],
            examples: &[
                ("fuzzming report",                               "Report for the current directory"),
                ("fuzzming report --workspace-root ./my-project", "Report for a different project"),
            ],
        },
        CommandDoc {
            name:      "config",
            one_liner: "View or reset the saved fuzzming.config",
            description: &[
                "Without flags: prints all saved keys, the API key is always masked.",
                "With --reset   deletes fuzzming.config so the next run re-prompts.",
            ],
            flags: &[
                ("--reset", "", "Delete fuzzming.config, next run will re-prompt for all settings"),
            ],
            examples: &[
                ("fuzzming config",         "View saved settings (API key masked)"),
                ("fuzzming config --reset", "Delete config and re-prompt on next run"),
            ],
        },
    ];

    // ── Overview ──────────────────────────────────────────────────────────────
    println!("{}", header.apply_to("  FUZZMING: AI-powered Solidity smart contract fuzzer"));
    println!("{}", dim.apply_to("  Point it at a Foundry project. Watch it think. Let it find bugs."));
    println!();
    println!("{}", dim.apply_to("  Usage:  fuzzming <SUBCOMMAND> [FLAGS]"));
    println!();

    // ── Subcommands ───────────────────────────────────────────────────────────
    println!("{}", header.apply_to("  SUBCOMMANDS"));
    println!("{}", sep.apply_to("  ──────────────────────────────────────────────────────────────────"));

    for cmd in commands {
        println!();

        // Name on its own line, one-liner on the same line
        println!("  {}   {}", label.apply_to(cmd.name), dim.apply_to(cmd.one_liner));

        // Description indented under the name
        for line in cmd.description {
            println!("    {}", dim.apply_to(*line));
        }
        println!();

        // Flags: content indented under label
        println!("    {}", dim.apply_to("Flags:"));
        if cmd.flags.is_empty() {
            println!("      {}", dim.apply_to("none"));
        } else {
            let col = cmd.flags.iter()
                .map(|(f, h, _)| f.len() + if h.is_empty() { 0 } else { h.len() + 1 })
                .max().unwrap_or(0);
            for (flag, hint, desc) in cmd.flags {
                print_flag_row("      ", flag, hint, desc, col);
            }
        }
        println!();

        // Examples: command and description on one aligned line
        println!("    {}", dim.apply_to("Examples:"));
        let col = cmd.examples.iter().map(|(ex, _)| ex.len()).max().unwrap_or(0);
        for (example, desc) in cmd.examples {
            println!("      {}   {}",
                hi.apply_to(format!("{:<col$}", example)),
                dim.apply_to(*desc),
            );
        }

        println!();
        println!("{}", sep.apply_to("  ──────────────────────────────────────────────────────────────────"));
    }

    // ── Global flags ──────────────────────────────────────────────────────────
    println!();
    println!("{}", header.apply_to("  GLOBAL FLAGS"));
    println!("{}", sep.apply_to("  ──────────────────────────────────────────────────────────────────"));
    println!();

    let global_flags: &[(&str, &str, &str)] = &[
        ("--help, -h", "", "Print this reference"),
        ("--version",  "", "Print the installed version"),
    ];
    let col = global_flags.iter()
        .map(|(f, h, _)| f.len() + if h.is_empty() { 0 } else { h.len() + 1 })
        .max().unwrap_or(0);
    for (flag, hint, desc) in global_flags {
        print_flag_row("  ", flag, hint, desc, col);
    }
    println!();
}

// ── demo mode ─────────────────────────────────────────────────────────────────

async fn run_demo() -> Result<()> {
    use crate::shared::models::{Fuzzer, Language, PromptMode, SessionConfig};
    use crate::shared::requests::session_request::SessionRequest;

    let demo_st = Style::new().fg(Color::Color256(220)).bold();
    let muted = Style::new().fg(Color::Color256(245));
    println!();
    println!("  {}  {}", demo_st.apply_to("◆ DEMO MODE"), muted.apply_to(", no LLM calls, no tokens spent"));
    println!("  {}", muted.apply_to("  3 mock contracts · scripted outcomes · real UI"));
    println!();

    let workspace_root = std::env::temp_dir().join(format!(
        "fuzzming-demo-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));

    let config = SessionConfig {
        model: "demo".to_string(),
        llm_key: String::new(),
        language: Language::Solidity,
        fuzzer: Fuzzer::Foundry,
        workspace_root,
        max_tokens: None,
        llm_timeout_secs: 0,
        full_coverage_rounds: 2,
        prompt_mode: PromptMode::Guided,
    };
    let request = SessionRequest {
        target_paths: vec![
            "src/TokenVault.sol".to_string(),
            "src/StakingPool.sol".to_string(),
            "src/PriceOracle.sol".to_string(),
        ],
        max_rounds: 3,
        config,
    };

    let orchestrator = DemoCompositionRoot::build();
    let outcomes = match orchestrator.run(request).await {
        Ok(o) => o,
        Err(err) => {
            let ui = CliUi::new();
            ui.error(&format!("Demo failed: {}", err));
            std::process::exit(1);
        }
    };

    print_outcome_reports(&outcomes);
    print_aggregate_summary(&outcomes);
    Ok(())
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
