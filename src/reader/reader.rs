use crate::interfaces::contexts::{ContractContext, CoverageContext, InvariantFiles};
use crate::llm::ports::LlmReaderPort;
use crate::reader::infrastructure::FileSystemReader;
use crate::reader::ports::{
    contract_reader_port::ContractReaderPort, coverage_reader_port::CoverageReaderPort,
};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct Reader {
    contract_reader: Arc<dyn ContractReaderPort>,
    coverage_reader: Arc<dyn CoverageReaderPort>,
    fs_reader: Arc<FileSystemReader>,
    invariant_files: InvariantFiles,
}

impl Reader {
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
impl LlmReaderPort for Reader {
    async fn get_contract_context(
        &self,
        path: &str,
        _include_comments: bool,
    ) -> Result<ContractContext> {
        let source_code = self.contract_reader.get_contract_context(path, false).await?;
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
}
