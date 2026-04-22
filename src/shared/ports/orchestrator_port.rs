use crate::shared::{requests::session_request::SessionRequest, responses::session_outcome::SessionOutcome};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait OrchestratorPort: Send + Sync {
    async fn run(&self, request: SessionRequest) -> Result<SessionOutcome>;
}
