use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use tokio::fs::{create_dir_all, write};

pub struct FileSystemWriter {
    base_path: PathBuf,
}

impl FileSystemWriter {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let full_path = self.base_path.join(path);
        let canonical_base = self.base_path.canonicalize().with_context(|| {
            format!("workspace root does not exist: {}", self.base_path.display())
        })?;
        // Resolve the target path without requiring it to exist yet by canonicalizing the parent.
        let parent = full_path.parent().unwrap_or(&full_path);
        create_dir_all(parent).await.with_context(|| {
            format!("failed to create directories for {}", full_path.display())
        })?;
        let canonical_parent = parent.canonicalize().with_context(|| {
            format!("failed to canonicalize parent of {}", full_path.display())
        })?;
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
