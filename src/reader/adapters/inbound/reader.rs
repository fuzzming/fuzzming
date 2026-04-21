use anyhow::Result;
use async_trait::async_trait;

use crate::reader::ports::inbound::ReaderRunPort;
use crate::shared::models::{ContractContext, CoverageContext, InvariantFiles};
use crate::shared::ports::ReaderPort;

pub struct Reader {
    use_case: Box<dyn ReaderRunPort>,
}

impl Reader {
    pub fn new(use_case: Box<dyn ReaderRunPort>) -> Self {
        Self { use_case }
    }
}

#[async_trait]
impl ReaderPort for Reader {
    async fn get_contract_context(&self, path: &str, include_comments: bool) -> Result<ContractContext> {
        self.use_case.get_contract_context(path, include_comments).await
    }

    async fn get_fuzz_output(&self) -> Result<Option<String>> {
        self.use_case.get_fuzz_output().await
    }

    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>> {
        self.use_case.get_coverage_context().await
    }

    async fn get_invariant_files(&self) -> Result<InvariantFiles> {
        self.use_case.get_invariant_files().await
    }
}
