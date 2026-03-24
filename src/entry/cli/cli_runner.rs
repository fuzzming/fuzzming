use anyhow::Result;
use crate::entry::cli::arg_parser::{parse_args, CliArgs};
use crate::interfaces::signals::SessionRequest;
use crate::interfaces::state::{SessionConfig, OutputFormat};
use crate::orchestrator::session_orchestrator::SessionOrchestrator;
use crate::composition::composition_root::CompositionRoot;

pub struct CliRunner;

impl CliRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) -> Result<()> {
        let args = parse_args();
        let config = SessionConfig {
            llm_url: args.llm_url.clone(),
            llm_key: args.llm_key.clone(),
            output_format: if args.ci_mode { OutputFormat::Ci } else { OutputFormat::Terminal },
            ci_mode: args.ci_mode,
        };
        let request = SessionRequest {
            target_paths: args.targets.clone(),
            max_rounds: args.max_rounds,
            config: config.clone(),
            output_format: if args.ci_mode { OutputFormat::Ci } else { OutputFormat::Terminal },
            ci_mode: args.ci_mode,
        };
        let orchestrator = CompositionRoot::build(config);
        let outcome = orchestrator.run(request).await?;
        Ok(())
    }
}

impl Default for CliRunner {
    fn default() -> Self {
        Self::new()
    }
}
