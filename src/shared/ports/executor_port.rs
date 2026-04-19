use crate::shared::models::ExecutorInput;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ExecutorPort: Send + Sync {
    async fn execute(&self, input: ExecutorInput) -> Result<()>;
}
