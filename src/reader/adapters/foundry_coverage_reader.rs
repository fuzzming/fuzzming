use crate::interfaces::contexts::CoverageContext;
use crate::reader::ports::coverage_reader_port::CoverageReaderPort;
use crate::reader::use_cases::parse_lcov;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

pub struct FoundryCoverageReader {
    base_path: PathBuf,
}

impl FoundryCoverageReader {
    pub fn new(base_path: String) -> Self {
        Self {
            base_path: PathBuf::from(base_path),
        }
    }

    async fn read_file(&self, path: &str) -> Result<String> {
        let full_path = self.base_path.join(path);
        fs::read_to_string(&full_path)
            .await
            .with_context(|| format!("failed to read file: {}", full_path.display()))
    }
}

#[async_trait]
impl CoverageReaderPort for FoundryCoverageReader {
    async fn read_coverage(&self, path: &str) -> Result<Option<CoverageContext>> {
        let raw_lcov = match self.read_file(path).await {
            Ok(s) => s,
            Err(_) => return Ok(None), // If the file doesn't exist, it's not an error, just no coverage.
        };

        let mut coverage = parse_lcov::parse_lcov(&raw_lcov)?;

        // Enrich gaps with source code context
        for gap in coverage.gaps.iter_mut() {
            if gap.file.is_empty() {
                continue;
            }
            if let Ok(source) = self.read_file(&gap.file).await {
                let lines: Vec<&str> = source.lines().collect();
                if lines.is_empty() {
                    continue;
                }
                let idx = (gap.line as isize - 1).max(0) as usize;
                let start = idx.saturating_sub(3);
                let end = std::cmp::min(idx + 3, lines.len().saturating_sub(1));
                let mut ctx = Vec::new();
                for (i, line) in lines.iter().enumerate().take(end + 1).skip(start) {
                    ctx.push(format!("{}: {}", i + 1, line));
                }
                gap.source_context = ctx;
            }
        }

        Ok(Some(coverage))
    }
}
