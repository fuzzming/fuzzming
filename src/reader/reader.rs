use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::contexts::{ContractContext, FuzzReportContext, CoverageContext, InvariantFiles};
use crate::llm::ports::LlmReaderPort;
use crate::fuzzer::ports::FuzzerReaderPort;
use crate::reader::infrastructure::FileSystemReader;

pub struct Reader {
    pub fs: FileSystemReader,
    pub invariant_files: InvariantFiles,
}

impl Reader {
    pub fn new(fs: FileSystemReader, invariant_files: InvariantFiles) -> Self {
        Self { fs, invariant_files }
    }
}

#[async_trait]
impl LlmReaderPort for Reader {
    async fn get_contract_context(&self) -> Result<ContractContext> {
        todo!()
    }

    async fn get_fuzz_report_context(&self) -> Result<Option<FuzzReportContext>> {
        todo!()
    }

    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>> {
        todo!()
    }
}

#[async_trait]
impl FuzzerReaderPort for Reader {
    async fn get_invariant_files(&self) -> Result<InvariantFiles> {
        todo!()
    }

    async fn get_fuzz_output(&self) -> Result<String> {
        todo!()
    }

    async fn get_lcov(&self) -> Result<String> {
        todo!()
    }
}
