use crate::orchestrator::orchestrator::SessionOrchestrator;
use crate::shared::models::SessionConfig;

pub struct CompositionRoot;

impl CompositionRoot {
    pub fn build(config: SessionConfig) -> SessionOrchestrator {
        todo!()
    }
}
