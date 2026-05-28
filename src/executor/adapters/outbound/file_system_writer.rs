use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use tokio::fs::{create_dir_all, write};

pub struct FileSystemWriter {
    base_path: PathBuf,
}

impl FileSystemWriter {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let full_path = self.base_path.join(path);
        let parent = full_path.parent().unwrap_or(&full_path);
        // Ensure parent directories exist before canonicalizing paths.
        create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create directories for {}", full_path.display()))?;
        let canonical_base = self.base_path.canonicalize().with_context(|| {
            format!(
                "failed to canonicalize workspace root: {}",
                self.base_path.display()
            )
        })?;
        let canonical_parent = parent
            .canonicalize()
            .with_context(|| format!("failed to canonicalize parent of {}", full_path.display()))?;
        if !canonical_parent.starts_with(&canonical_base) {
            bail!(
                "path traversal rejected: '{}' escapes workspace root '{}'",
                full_path.display(),
                canonical_base.display()
            );
        }
        write(&full_path, content)
            .await
            .with_context(|| format!("failed to write {}", full_path.display()))
    }
}
