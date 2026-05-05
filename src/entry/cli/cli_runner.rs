use crate::composition::composition_root::CompositionRoot;
use crate::entry::cli::arg_parser::parse_args;
use crate::shared::models::{Fuzzer, Language, OutputFormat, SessionConfig};
use crate::shared::requests::session_request::SessionRequest;
use crate::shared::responses::session_outcome::TerminationReason;
use anyhow::Result;

pub struct CliRunner;

impl CliRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) -> Result<()> {
        let args = parse_args();
        let config = SessionConfig {
            model: args.model.clone(),
            llm_key: args.llm_key.clone(),
            output_format: if args.ci_mode {
                OutputFormat::Ci
            } else {
                OutputFormat::Terminal
            },
            ci_mode: args.ci_mode,
            language: Language::Solidity,
            fuzzer: Fuzzer::Foundry,
            workspace_root: args.workspace_root.clone(),
        };
        let request = SessionRequest {
            target_paths: args.targets.clone(),
            max_rounds: args.max_rounds,
            config: config.clone(),
            output_format: if args.ci_mode {
                OutputFormat::Ci
            } else {
                OutputFormat::Terminal
            },
            ci_mode: args.ci_mode,
        };
        let orchestrator = CompositionRoot::build(config);
        let outcome = orchestrator.run(request).await?;

        let has_bugs = matches!(outcome.reason, TerminationReason::Bug | TerminationReason::DevTestFailed)
            || !outcome.artifacts.call_sequences.is_empty();

        if has_bugs {
            std::process::exit(1);
        }

        Ok(())
    }
}

impl Default for CliRunner {
    fn default() -> Self {
        Self::new()
    }
}
