use anyhow::Result;
use async_trait::async_trait;

use crate::shared::responses::stage_event::StageEvent;

#[async_trait]
pub trait OutputPort: Send + Sync {
    async fn write(&self, output: &str) -> Result<()>;
    async fn write_progress(&self, output: &str) -> Result<()>;
    async fn handle_stage_event(&self, event: StageEvent) -> Result<()>;
}
