use crate::fuzzer::ports::FuzzerReaderPort;
use crate::interfaces::contexts::{ContractContext, CoverageContext, FuzzReportContext, InvariantFiles};
use crate::llm::ports::LlmReaderPort;
use crate::reader::ports::{contract_reader_port::ContractReaderPort, coverage_reader_port::CoverageReaderPort};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// The main orchestrator for the reader module.
/// It holds references to the various reader ports and delegates calls to them.
pub struct Reader {
    contract_reader: Arc<dyn ContractReaderPort>,
    coverage_reader: Arc<dyn CoverageReaderPort>,
    invariant_files: InvariantFiles,
}

impl Reader {
    pub fn new(
        contract_reader: Arc<dyn ContractReaderPort>,
        coverage_reader: Arc<dyn CoverageReaderPort>,
        invariant_files: InvariantFiles,
    ) -> Self {
        Self {
            contract_reader,
            coverage_reader,
            invariant_files,
        }
    }
}

#[async_trait]
impl LlmReaderPort for Reader {
    async fn get_contract_context(&self, path: &str, _include_comments: bool) -> Result<ContractContext> {
        let source_code = self.contract_reader.get_contract_context(path, true).await?;
        crate::reader::use_cases::parse_contract::parse_contract(&source_code)
    }

    async fn get_fuzz_report_context(&self) -> Result<Option<FuzzReportContext>> {
        // This would be implemented by a new FuzzReportReader port and adapter
        Ok(None)
    }

    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>> {
        self.coverage_reader
            .read_coverage(&self.invariant_files.lcov_path)
            .await
    }
}

#[async_trait]
impl FuzzerReaderPort for Reader {
    async fn get_invariant_files(&self) -> Result<InvariantFiles> {
        Ok(self.invariant_files.clone())
    }

    async fn get_fuzz_output(&self) -> Result<String> {
        // This would be implemented by a generic file reader port/adapter
        // For now, we can't implement this without re-introducing a generic file reader.
        // This highlights a design decision to be made.
        unimplemented!("FuzzerReaderPort::get_fuzz_output is not implemented in the new architecture yet.")
    }

    async fn get_lcov(&self) -> Result<String> {
        unimplemented!("FuzzerReaderPort::get_lcov is not implemented in the new architecture yet.")
    }
}
