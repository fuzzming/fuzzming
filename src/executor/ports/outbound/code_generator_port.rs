use crate::executor::adapters::outbound::FileSystemWriter;
use crate::shared::models::BodiesJson;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait CodeGeneratorPort: Send + Sync {
    async fn generate(&self, bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()>;
}
