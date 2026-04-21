use crate::executor::adapters::outbound::FileSystemWriter;
use crate::shared::models::BodiesJson;
use anyhow::Result;

pub async fn write_bodies(bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
    let path = format!("test/{}.bodies.json", bodies.meta.contract);
    let content = serde_json::to_string_pretty(bodies)?;
    writer.write_file(&path, &content).await
}
