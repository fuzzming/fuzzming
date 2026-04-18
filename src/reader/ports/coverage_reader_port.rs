use crate::interfaces::contexts::CoverageContext;
use anyhow::Result;
use async_trait::async_trait;

/// A port for reading and parsing coverage data.
#[async_trait]
pub trait CoverageReaderPort: Send + Sync {
    async fn read_coverage(&self, path: &str) -> Result<Option<CoverageContext>>;
}
