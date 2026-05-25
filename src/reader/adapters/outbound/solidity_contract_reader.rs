use crate::reader::adapters::outbound::FileSystemReader;
use crate::reader::ports::outbound::ContractReaderPort;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct SolidityContractReader {
    reader: Arc<FileSystemReader>,
}

impl SolidityContractReader {
    pub fn new(reader: Arc<FileSystemReader>) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl ContractReaderPort for SolidityContractReader {
    async fn get_contract_context(&self, path: &str, include_comments: bool) -> Result<String> {
        let content = self.reader.read_file(path).await?;

        let filtered: Vec<&str> = content
            .lines()
            .filter(|l| !l.trim().starts_with("pragma"))
            .collect();
        let mut source = filtered.join("\n");

        if !include_comments {
            let block_re = regex::Regex::new(r"(?s)/\*.*?\*/").unwrap();
            source = block_re.replace_all(&source, "").to_string();

            let line_re = regex::Regex::new(r"//.*").unwrap();
            source = line_re.replace_all(&source, "").to_string();

            source = source
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect::<Vec<&str>>()
                .join("\n");
        }

        Ok(source)
    }
}
