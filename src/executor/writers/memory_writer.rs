use anyhow::Result;
use crate::reader::parsers::memory_parser::MemoryEntry;
use crate::executor::infrastructure::FileSystemWriter;

pub async fn write_memory(
    entries: &[MemoryEntry],
    writer: &FileSystemWriter,
) -> Result<()> {
    todo!()
}
