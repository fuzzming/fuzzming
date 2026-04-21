use crate::executor::adapters::outbound::FileSystemWriter;
use crate::shared::models::FuzzerConfigArtifact;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ConfigWriterPort: Send + Sync {
    async fn write(&self, config: &FuzzerConfigArtifact, writer: &FileSystemWriter) -> Result<()>;
}
