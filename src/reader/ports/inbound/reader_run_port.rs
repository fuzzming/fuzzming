use crate::shared::models::{BodiesJson, ContractContext, CoverageContext};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ReaderRunPort: Send + Sync {
    async fn get_contract_context(
        &self,
        path: &str,
        include_comments: bool,
    ) -> Result<ContractContext>;
    async fn get_fuzz_output(&self, path: &str) -> Result<Option<String>>;
    async fn get_coverage_context(&self, path: &str) -> Result<Option<CoverageContext>>;
    async fn get_existing_bodies(&self, path: &str) -> Result<Option<BodiesJson>>;
}
