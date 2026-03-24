use anyhow::Result;
use crate::interfaces::artifacts::InvariantSet;
use crate::executor::infrastructure::FileSystemWriter;

pub async fn write_invariants(
    set: &InvariantSet,
    writer: &FileSystemWriter,
) -> Result<()> {
    todo!()
}
