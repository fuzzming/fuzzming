use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::reader::adapters::outbound::FileSystemReader;
use crate::reader::ports::inbound::ReaderRunPort;
use crate::reader::ports::outbound::{ContractReaderPort, CoverageReaderPort};
use crate::shared::models::{BodiesJson, ContractContext, CoverageContext};

pub struct ReadUseCase {
    contract_reader: Arc<dyn ContractReaderPort>,
    coverage_reader: Arc<dyn CoverageReaderPort>,
    fs_reader: Arc<FileSystemReader>,
}

impl ReadUseCase {
    pub fn new(
        contract_reader: Arc<dyn ContractReaderPort>,
        coverage_reader: Arc<dyn CoverageReaderPort>,
        fs_reader: Arc<FileSystemReader>,
    ) -> Self {
        Self {
            contract_reader,
            coverage_reader,
            fs_reader,
        }
    }
}

#[async_trait]
impl ReaderRunPort for ReadUseCase {
    async fn get_contract_context(
        &self,
        path: &str,
        _include_comments: bool,
    ) -> Result<ContractContext> {
        let source_code = self
            .contract_reader
            .get_contract_context(path, false)
            .await?;
        Ok(ContractContext { source_code })
    }

    async fn get_fuzz_output(&self, path: &str) -> Result<Option<String>> {
        self.fs_reader.read_file_optional(path).await
    }

    async fn get_coverage_context(&self, path: &str) -> Result<Option<CoverageContext>> {
        self.coverage_reader.read_coverage(path).await
    }

    async fn get_existing_bodies(&self, path: &str) -> Result<Option<BodiesJson>> {
        match self.fs_reader.read_file_optional(path).await? {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }
}
