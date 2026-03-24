use anyhow::Result;

pub struct FileSystemReader {
    pub base_path: String,
}

impl FileSystemReader {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }

    pub async fn read_file(&self, path: &str) -> Result<String> {
        todo!()
    }
}
