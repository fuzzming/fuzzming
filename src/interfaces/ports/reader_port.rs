use crate::interfaces::contexts::{ContractContext, CoverageContext, InvariantFiles};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ReaderPort: Send + Sync {
    async fn get_contract_context(
        &self,
        path: &str,
        _include_comments: bool,
    ) -> Result<ContractContext>;
    async fn get_fuzz_output(&self) -> Result<Option<String>>;
    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>>;
    async fn get_invariant_files(&self) -> Result<InvariantFiles>;
}
