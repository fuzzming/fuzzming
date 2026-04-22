use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait CoverageReaderPort: Send + Sync {
    async fn read_lcov(&self, path: &str) -> Result<Option<String>>;
}
