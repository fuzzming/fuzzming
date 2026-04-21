use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ContractReaderPort: Send + Sync {
    async fn get_contract_context(&self, path: &str, include_comments: bool) -> Result<String>;
}
