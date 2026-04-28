use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::fs;

pub struct FileSystemReader {
    base_path: PathBuf,
}

impl FileSystemReader {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub async fn read_file(&self, path: &str) -> Result<String> {
        let full_path = self.base_path.join(path);
        fs::read_to_string(&full_path)
            .await
            .with_context(|| format!("failed to read file: {}", full_path.display()))
    }

    pub async fn read_file_optional(&self, path: &str) -> Result<Option<String>> {
        match self.read_file(path).await {
            Ok(s) => Ok(Some(s)),
            Err(_) => Ok(None),
        }
    }
}
