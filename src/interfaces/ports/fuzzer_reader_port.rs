use crate::interfaces::contexts::InvariantFiles;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait FuzzerReaderPort: Send + Sync {
    async fn get_invariant_files(&self) -> Result<InvariantFiles>;
    async fn get_fuzz_output(&self) -> Result<String>;
    async fn get_lcov(&self) -> Result<String>;
}
