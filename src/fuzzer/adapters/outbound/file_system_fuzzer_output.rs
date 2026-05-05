use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use tokio::fs;

use crate::fuzzer::ports::outbound::FuzzerOutputPort;

const FUZZMING_DIR: &str = ".fuzzming";

pub struct FileSystemFuzzerOutput {
    workspace_root: PathBuf,
}

impl FileSystemFuzzerOutput {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl FuzzerOutputPort for FileSystemFuzzerOutput {
    async fn write_fuzz_output(&self, contract_name: &str, content: &str) -> Result<()> {
        let dir = self.workspace_root.join(FUZZMING_DIR).join(contract_name);
        fs::create_dir_all(&dir).await?;
        fs::write(dir.join("fuzz_output.txt"), content).await?;
        Ok(())
    }

    async fn write_lcov(&self, contract_name: &str, content: &str) -> Result<PathBuf> {
        let dir = self.workspace_root.join(FUZZMING_DIR).join(contract_name);
        fs::create_dir_all(&dir).await?;
        let dest = dir.join("lcov.info");
        fs::write(&dest, content).await?;
        Ok(dest)
    }
}
