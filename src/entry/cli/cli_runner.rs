use crate::composition::composition_root::CompositionRoot;
use crate::entry::cli::arg_parser::{parse_args, Command};
use crate::entry::cli::interactive::resolve_cli_config;
use crate::entry::cli::ui::CliUi;
use crate::shared::models::{Fuzzer, Language, OutputFormat, SessionConfig};
use crate::shared::requests::session_request::SessionRequest;
use crate::shared::responses::session_outcome::TerminationReason;
use anyhow::Result;
use tracing_subscriber::EnvFilter;

pub struct CliRunner;

impl CliRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) -> Result<()> {
        let args = parse_args();
        init_tracing(args.verbose);
        let ui = CliUi::new();

        if let Some(Command::Guide) = args.command {
            print_extended_help(&ui);
            return Ok(());
        }

        let resolved = resolve_cli_config(&args)?;

        let output_format = if resolved.ci_mode {
            OutputFormat::Ci
        } else {
            OutputFormat::Terminal
        };

        let config = SessionConfig {
            model: resolved.model.clone(),
            llm_key: resolved.llm_key.clone(),
            output_format: output_format.clone(),
            ci_mode: resolved.ci_mode,
            language: Language::Solidity,
            fuzzer: Fuzzer::Foundry,
            workspace_root: resolved.workspace_root.clone(),
        };
        let request = SessionRequest {
            target_paths: resolved.targets.clone(),
            max_rounds: resolved.max_rounds,
            config: config.clone(),
            output_format,
            ci_mode: resolved.ci_mode,
        };
        let orchestrator = CompositionRoot::build(config);

        let outcome = match orchestrator.run(request).await {
            Ok(outcome) => outcome,
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

        let has_bugs = matches!(
            outcome.reason,
            TerminationReason::Bug | TerminationReason::DevTestFailed
        ) || !outcome.artifacts.call_sequences.is_empty();

        if has_bugs {
            std::process::exit(1);
        }

        Ok(())
    }
}

fn print_extended_help(ui: &CliUi) {
    ui.banner();
    ui.divider();
    ui.info("Commands:");
    ui.info("  guide                    Show this guide and examples");
    ui.divider();
    ui.info("Flags:");
    ui.info("  --targets <PATHS...>     Paths to target contracts");
    ui.info("  --max-rounds <N>         Maximum number of fuzzing rounds");
    ui.info("  --model <ID>             LLM model identifier (env: LLM_MODEL)");
    ui.info("  --llm-key <KEY>          LLM API key (env: LLM_KEY)");
    ui.info("  --workspace-root <DIR>   Foundry project root");
    ui.info("  --interactive            Force interactive prompts");
    ui.info("  --ci-mode                CI-friendly output");
    ui.info("  --verbose                Enable verbose logs");
    ui.divider();
    ui.info("Examples:");
    ui.info("  fuzzming --interactive");
    ui.info("  fuzzming --workspace-root ./project --targets src/Vault.sol");
    ui.info("  fuzzming --model groq/llama-3.3-70b-versatile --llm-key $LLM_KEY");
    ui.info("  fuzzming guide");
    println!();
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
