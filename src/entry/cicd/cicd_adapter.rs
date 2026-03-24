use anyhow::Result;
use crate::entry::cicd::env_reader::{read_cicd_env, CicdEnv};
use crate::interfaces::signals::SessionRequest;
use crate::interfaces::state::{SessionConfig, OutputFormat};
use crate::orchestrator::session_orchestrator::SessionOrchestrator;
use crate::composition::composition_root::CompositionRoot;

pub struct CicdAdapter;

impl CicdAdapter {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) -> Result<()> {
        let env = read_cicd_env()?;
        let config = SessionConfig {
            llm_url: env.llm_url.clone(),
            llm_key: env.llm_key.clone(),
            output_format: OutputFormat::Ci,
            ci_mode: true,
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
