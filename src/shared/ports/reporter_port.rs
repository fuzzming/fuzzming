use anyhow::Result;
use async_trait::async_trait;

use crate::shared::responses::{
    round_usage::RoundUsage, session_outcome::SessionOutcome, stage_event::StageEvent,
};

#[async_trait]
pub trait ReporterPort: Send + Sync {
    async fn emit(&self, outcome: SessionOutcome) -> Result<()>;
    async fn emit_round_usage(&self, usage: RoundUsage) -> Result<()>;
    async fn emit_stage_event(&self, event: StageEvent) -> Result<()>;
}
