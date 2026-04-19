use crate::reader::infrastructure::FileSystemReader;
use crate::reader::ports::coverage_reader_port::CoverageReaderPort;
use crate::reader::use_cases::parse_lcov;
use crate::shared::models::CoverageContext;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct FoundryCoverageReader {
    reader: Arc<FileSystemReader>,
}

impl FoundryCoverageReader {
    pub fn new(reader: Arc<FileSystemReader>) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl CoverageReaderPort for FoundryCoverageReader {
    async fn read_coverage(&self, path: &str) -> Result<Option<CoverageContext>> {
        let raw_lcov = match self.reader.read_file_optional(path).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        let mut coverage = parse_lcov::parse_lcov(&raw_lcov)?;

        for gap in coverage.gaps.iter_mut() {
            if gap.file.is_empty() {
                continue;
            }
            if let Ok(source) = self.reader.read_file(&gap.file).await {
                let lines: Vec<&str> = source.lines().collect();
                if lines.is_empty() {
                    continue;
                }
                let idx = (gap.line as isize - 1).max(0) as usize;
                let start = idx.saturating_sub(3);
                let end = std::cmp::min(idx + 3, lines.len().saturating_sub(1));
                gap.source_context = lines
                    .iter()
                    .enumerate()
                    .take(end + 1)
                    .skip(start)
                    .map(|(i, line)| format!("{}: {}", i + 1, line))
                    .collect();
            }
        }

        Ok(Some(coverage))
    }
}
