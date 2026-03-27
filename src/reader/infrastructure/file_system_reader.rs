use anyhow::Result;

pub struct FileSystemReader {
    pub base_path: String,
}

impl FileSystemReader {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }

    pub async fn read_file(&self, path: &str) -> Result<String> {
        use tokio::fs;
        use anyhow::Context;

        let full = if std::path::Path::new(path).is_absolute() {
            path.to_string()
        } else {
            format!("{}/{}", self.base_path.trim_end_matches('/'), path)
        };

        let data = fs::read_to_string(&full).await
            .with_context(|| format!("failed reading file: {}", full))?;

        Ok(data)
    }
}
