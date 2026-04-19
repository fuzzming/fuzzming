use crate::shared::models::CoverageContext;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait CoverageReaderPort: Send + Sync {
    async fn read_coverage(&self, path: &str) -> Result<Option<CoverageContext>>;
}
