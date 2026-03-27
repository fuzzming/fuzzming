use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::contexts::{ContractContext, FuzzReportContext, CoverageContext};

#[async_trait]
pub trait LlmReaderPort: Send + Sync {
    /// Read and parse a Solidity contract located at `path`.
    /// If `include_comments` is true, comments will be preserved (parser may optionally use them).
    async fn get_contract_context(&self, path: &str, include_comments: bool) -> Result<ContractContext>;
    async fn get_fuzz_report_context(&self) -> Result<Option<FuzzReportContext>>;
    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>>;
}
