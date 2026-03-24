use crate::interfaces::state::SessionConfig;
use crate::orchestrator::session_orchestrator::SessionOrchestrator;

pub struct CompositionRoot;

impl CompositionRoot {
    pub fn build(config: SessionConfig) -> SessionOrchestrator {
        todo!()
    }
}
