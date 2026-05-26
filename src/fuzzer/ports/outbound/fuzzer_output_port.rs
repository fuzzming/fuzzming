use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;

use crate::shared::models::CoverageContext;

#[async_trait]
pub trait FuzzerOutputPort: Send + Sync {
    async fn write_fuzz_output(&self, contract_name: &str, content: &str) -> Result<()>;
    async fn write_lcov(&self, contract_name: &str, content: &str) -> Result<PathBuf>;
    async fn write_coverage_context(&self, contract_name: &str, context: &CoverageContext) -> Result<()>;
}
