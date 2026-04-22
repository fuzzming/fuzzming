use crate::orchestrator::adapters::inbound::Orchestrator;
use crate::shared::models::SessionConfig;
use crate::shared::ports::OrchestratorPort;

pub struct CompositionRoot;

impl CompositionRoot {
    pub fn build(_config: SessionConfig) -> Box<dyn OrchestratorPort> {
        todo!()
    }
}
