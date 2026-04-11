use crate::executor::infrastructure::FileSystemWriter;
use crate::interfaces::artifacts::BodiesJson;
use anyhow::Result;

/// Serialise `bodies` to `test/<contract>.bodies.json`.
/// The path is always `test/<ContractName>.bodies.json` relative to the workspace root.
pub async fn write_bodies(bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
    let path = format!("test/{}.bodies.json", bodies.meta.contract);
    let content = serde_json::to_string_pretty(bodies)?;
    writer.write_file(&path, &content).await
}
