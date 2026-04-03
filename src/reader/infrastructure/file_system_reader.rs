use anyhow::Result;

pub struct FileSystemReader {
    pub base_path: String,
}

impl FileSystemReader {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }

    /// Read a file as a single String. Does not split lines.
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

    /// Read a contract file as a single String. If `include_comments` is false,
    /// block and line comments are stripped before returning.
    /// Pragma and import statements are always stripped.
    pub async fn read_contract(&self, path: &str, include_comments: bool) -> Result<String> {
        let content = self.read_file(path).await?;
        
        // Strip pragma and import lines
        let lines: Vec<&str> = content.lines().collect();
        let filtered: Vec<&str> = lines.iter()
            .filter(|line| !line.trim().starts_with("pragma") && !line.trim().starts_with("import"))
            .copied()
            .collect();
        let without_imports = filtered.join("\n");
        
        if include_comments {
            return Ok(without_imports);
        }

        // Strip block and line comments.
        // Use regex here to keep behaviour consistent with parsers.
        use regex::Regex;

        let re_block = Regex::new(r"/\*[\s\S]*?\*/").unwrap();
        let re_line = Regex::new(r"//.*").unwrap();
        let tmp = re_block.replace_all(&without_imports, "");
        let cleaned = re_line.replace_all(&tmp, "").to_string();
        Ok(cleaned)
    }
}
