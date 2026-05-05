use crate::composition::composition_root::CompositionRoot;
use crate::entry::cicd::env_reader::read_cicd_env;
use crate::shared::models::{Fuzzer, Language, OutputFormat, SessionConfig};
use crate::shared::requests::session_request::SessionRequest;
use anyhow::Result;

pub struct CicdAdapter;

impl CicdAdapter {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) -> Result<()> {
        let env = read_cicd_env()?;
        let config = SessionConfig {
            model: env.model.clone(),
            llm_key: env.llm_key.clone(),
            output_format: OutputFormat::Ci,
            ci_mode: true,
            language: Language::Solidity,
            fuzzer: Fuzzer::Foundry,
            workspace_root: std::env::var("WORKSPACE_ROOT").unwrap_or_else(|_| ".".to_string()).into(),
        };
        let request = SessionRequest {
            target_paths: env.target_paths.clone(),
            max_rounds: env.max_rounds,
            config: config.clone(),
            output_format: OutputFormat::Ci,
            ci_mode: true,
        };
        let orchestrator = CompositionRoot::build(config);
        let _outcome = orchestrator.run(request).await?;
        Ok(())
    }
}

impl Default for CicdAdapter {
    fn default() -> Self {
        Self::new()
    }
}
