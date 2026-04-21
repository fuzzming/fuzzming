use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::reader::adapters::outbound::FileSystemReader;
use crate::reader::ports::inbound::ReaderRunPort;
use crate::reader::ports::outbound::{ContractReaderPort, CoverageReaderPort};
use crate::shared::models::{ContractContext, CoverageContext, InvariantFiles};

pub struct ReadUseCase {
    contract_reader: Arc<dyn ContractReaderPort>,
    coverage_reader: Arc<dyn CoverageReaderPort>,
    fs_reader: Arc<FileSystemReader>,
    invariant_files: InvariantFiles,
}

impl ReadUseCase {
    pub fn new(
        contract_reader: Arc<dyn ContractReaderPort>,
        coverage_reader: Arc<dyn CoverageReaderPort>,
        fs_reader: Arc<FileSystemReader>,
        invariant_files: InvariantFiles,
    ) -> Self {
        Self {
            contract_reader,
            coverage_reader,
            fs_reader,
            invariant_files,
        }
    }
}

#[async_trait]
impl ReaderRunPort for ReadUseCase {
    async fn get_contract_context(&self, path: &str, _include_comments: bool) -> Result<ContractContext> {
        let source_code = self
            .contract_reader
            .get_contract_context(path, false)
            .await?;
        Ok(ContractContext { source_code })
    }

    async fn get_fuzz_output(&self) -> Result<Option<String>> {
        self.fs_reader
            .read_file_optional(&self.invariant_files.fuzz_output_path)
            .await
    }

    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>> {
        self.coverage_reader
            .read_coverage(&self.invariant_files.lcov_path)
            .await
    }

    async fn get_invariant_files(&self) -> Result<InvariantFiles> {
        Ok(self.invariant_files.clone())
    }
}
