use crate::interfaces::artifacts::{BodiesJson, FoundryConfig};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ExecutorPort: Send + Sync {
    async fn write_bodies(&self, bodies: BodiesJson) -> Result<()>;

    async fn write_foundry_config(&self, config: FoundryConfig) -> Result<()>;
}
