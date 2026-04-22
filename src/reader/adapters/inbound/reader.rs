use anyhow::Result;
use async_trait::async_trait;

use crate::reader::ports::inbound::ReaderRunPort;
use crate::shared::models::{BodiesJson, ContractContext, CoverageContext};
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
    async fn get_contract_context(
        &self,
        path: &str,
        include_comments: bool,
    ) -> Result<ContractContext> {
        self.use_case
            .get_contract_context(path, include_comments)
            .await
    }

    async fn get_fuzz_output(&self, path: &str) -> Result<Option<String>> {
        self.use_case.get_fuzz_output(path).await
    }

    async fn get_coverage_context(&self, path: &str) -> Result<Option<CoverageContext>> {
        self.use_case.get_coverage_context(path).await
    }

    async fn get_existing_bodies(&self, path: &str) -> Result<Option<BodiesJson>> {
        self.use_case.get_existing_bodies(path).await
    }
}
