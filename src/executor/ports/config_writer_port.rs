use crate::executor::infrastructure::FileSystemWriter;
use crate::interfaces::artifacts::FuzzerConfigArtifact;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ConfigWriterPort: Send + Sync {
    async fn write(&self, config: &FuzzerConfigArtifact, writer: &FileSystemWriter) -> Result<()>;
}
