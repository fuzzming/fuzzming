use crate::reader::adapters::outbound::FileSystemReader;
use crate::reader::ports::outbound::CoverageReaderPort;
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
    async fn read_lcov(&self, path: &str) -> Result<Option<String>> {
        self.reader.read_file_optional(path).await
    }
}
