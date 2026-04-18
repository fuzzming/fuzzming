use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait FuzzerReaderPort: Send + Sync {
    /// Reads fuzzer output from a JSON file and parses the failures
    /// into a descriptive summary string.
    async fn read_fuzzer_output(&self, path: &str) -> Result<String>;
}
