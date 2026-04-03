use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::contexts::{ContractContext, FuzzReportContext, CoverageContext, InvariantFiles};
use crate::llm::ports::LlmReaderPort;
use crate::fuzzer::ports::FuzzerReaderPort;
use crate::reader::infrastructure::FileSystemReader;
use crate::reader::parsers::{contract_parser, lcov_parser};
use anyhow::Context;

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
    async fn get_contract_context(&self, path: &str, include_comments: bool) -> Result<ContractContext> {
        let data = self.fs.read_contract(path, include_comments).await.context(format!("reading contract file {}", path))?;
        // The FileSystemReader returns the full file contents (comments stripped if requested).
        let ctx = contract_parser::parse_contract(&data, include_comments)?;
        Ok(ctx)
    }

    async fn get_fuzz_report_context(&self) -> Result<Option<FuzzReportContext>> {
        // Fuzz output parsing removed — Reader no longer parses forge output here.
        Ok(None)
    }

    async fn get_coverage_context(&self) -> Result<Option<CoverageContext>> {
        let path = &self.invariant_files.lcov_path;
        let raw = match self.fs.read_file(path).await {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };

        let mut coverage = lcov_parser::parse_lcov(&raw)?;

        // Enrich: for each gap attempt to read the file and provide 3 lines either side
        for gap in coverage.gaps.iter_mut() {
            if gap.file.is_empty() { continue; }
            if let Ok(source) = self.fs.read_file(&gap.file).await {
                let lines: Vec<&str> = source.lines().collect();
                if lines.is_empty() { continue; }
                let idx = (gap.line as isize - 1).max(0) as usize;
                let start = idx.saturating_sub(3);
                let end = std::cmp::min(idx + 3, lines.len().saturating_sub(0));
                let mut ctx = Vec::new();
                for i in start..=end { if i < lines.len() { ctx.push(format!("{}: {}", i+1, lines[i])); } }
                gap.source_context = ctx;
            }
        }

        Ok(Some(coverage))
    }
}

#[async_trait]
impl FuzzerReaderPort for Reader {
    async fn get_invariant_files(&self) -> Result<InvariantFiles> {
        Ok(self.invariant_files.clone())
    }

    async fn get_fuzz_output(&self) -> Result<String> {
        let p = &self.invariant_files.fuzz_output_path;
        let data = self.fs.read_file(p).await?;
        Ok(data)
    }

    async fn get_lcov(&self) -> Result<String> {
        let p = &self.invariant_files.lcov_path;
        let data = self.fs.read_file(p).await?;
        Ok(data)
    }
}
