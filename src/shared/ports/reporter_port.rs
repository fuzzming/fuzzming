use anyhow::Result;
use async_trait::async_trait;

use crate::shared::responses::session_outcome::SessionOutcome;

#[async_trait]
pub trait ReporterPort: Send + Sync {
    async fn emit(&self, outcome: SessionOutcome) -> Result<()>;
}
