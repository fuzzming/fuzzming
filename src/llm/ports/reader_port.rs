use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::contexts::{ContractContext, CoverageContext, FuzzReportContext};

#[async_trait]
pub trait LlmReaderPort: Send + Sync {
    async fn get_contract_context(&self) -> Result<ContractContext>;
    async fn get_fuzz_report_context(&self) -> Result<Option<FuzzReportContext>>;
    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>>;
}
