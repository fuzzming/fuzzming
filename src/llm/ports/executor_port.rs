use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::artifacts::{InvariantSet, FoundryConfig};

#[async_trait]
pub trait ExecutorPort: Send + Sync {
    async fn write_invariants(&self, set: InvariantSet) -> Result<()>;
    async fn write_foundry_config(&self, config: FoundryConfig) -> Result<()>;
}
