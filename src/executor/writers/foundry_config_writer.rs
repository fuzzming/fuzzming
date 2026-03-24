use anyhow::Result;
use crate::interfaces::artifacts::FoundryConfig;
use crate::executor::infrastructure::FileSystemWriter;

pub async fn write_foundry_config(
    config: &FoundryConfig,
    writer: &FileSystemWriter,
) -> Result<()> {
    todo!()
}
