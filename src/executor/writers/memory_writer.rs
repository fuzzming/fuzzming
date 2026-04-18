use anyhow::Result;
use crate::interfaces::contexts::ContractContext; // Replacing missing MemoryEntry
use crate::executor::infrastructure::FileSystemWriter;

pub async fn write_memory(
    entries: &[ContractContext],
    writer: &FileSystemWriter,
) -> Result<()> {
    todo!()
}
