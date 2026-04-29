use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait OutputPort: Send + Sync {
    async fn write(&self, output: &str) -> Result<()>;
}
