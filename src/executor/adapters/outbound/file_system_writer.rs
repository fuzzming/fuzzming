use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs::{create_dir_all, write};

pub struct FileSystemWriter {
    base_path: PathBuf,
}

impl FileSystemWriter {
    pub fn new(base_path: String) -> Self {
        Self {
            base_path: PathBuf::from(base_path),
        }
    }

    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let full_path = self.base_path.join(path);
        if let Some(parent) = full_path.parent() {
            create_dir_all(parent).await.with_context(|| {
                format!("failed to create directories for {}", full_path.display())
            })?;
        }
        write(&full_path, content)
            .await
            .with_context(|| format!("failed to write {}", full_path.display()))
    }
}
