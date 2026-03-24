use anyhow::Result;

pub struct FileSystemWriter {
    pub base_path: String,
}

impl FileSystemWriter {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }

    pub async fn write_file(&self, path: &str, content: &str) -> Result<()> {
        todo!()
    }
}
