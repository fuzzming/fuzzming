use anyhow::Result;
use async_trait::async_trait;

use crate::orchestrator::ports::inbound::OrchestratorRunPort;
use crate::shared::ports::OrchestratorPort;
use crate::shared::requests::session_request::SessionRequest;
use crate::shared::responses::session_outcome::SessionOutcome;

pub struct Orchestrator {
    use_case: Box<dyn OrchestratorRunPort>,
}

impl Orchestrator {
    pub fn new(use_case: Box<dyn OrchestratorRunPort>) -> Self {
        Self { use_case }
    }
}

#[async_trait]
impl OrchestratorPort for Orchestrator {
    async fn run(&self, request: SessionRequest) -> Result<SessionOutcome> {
        self.use_case.run(request).await
    }
}
