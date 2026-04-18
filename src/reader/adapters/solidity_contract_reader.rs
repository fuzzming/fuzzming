use crate::reader::ports::contract_reader_port::ContractReaderPort;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

pub struct SolidityContractReader {
    base_path: PathBuf,
}

impl SolidityContractReader {
    pub fn new(base_path: String) -> Self {
        Self {
            base_path: PathBuf::from(base_path),
        }
    }

    async fn read_file(&self, path: &str) -> Result<String> {
        let full_path = self.base_path.join(path);
        fs::read_to_string(&full_path)
            .await
            .with_context(|| format!("failed to read file: {}", full_path.display()))
    }
}

#[async_trait]
impl ContractReaderPort for SolidityContractReader {
    async fn get_contract_context(&self, path: &str, include_comments: bool) -> Result<String> {
        let content = self
            .read_file(path)
            .await
            .with_context(|| format!("Failed to read contract file: {}", path))?;

        // Pre-process the content: strip pragma and import statements
        let lines: Vec<&str> = content.lines().collect();
        let filtered: Vec<&str> = lines
            .iter()
            .filter(|line| !line.trim().starts_with("pragma") && !line.trim().starts_with("import"))
            .copied()
            .collect();
        let mut without_imports = filtered.join("\n");

        if !include_comments {
            // Remove block comments /* ... */
            let block_re = regex::Regex::new(r"(?s)/\*.*?\*/").unwrap();
            without_imports = block_re.replace_all(&without_imports, "").to_string();
            
            // Remove line comments // ...
            let line_re = regex::Regex::new(r"//.*").unwrap();
            without_imports = line_re.replace_all(&without_imports, "").to_string();
            
            // Optional: trim trailing whitespaces left by the removal
            without_imports = without_imports.lines()
                .filter(|line| !line.trim().is_empty())
                .collect::<Vec<&str>>()
                .join("\n");
        }

        Ok(without_imports)
    }
}
