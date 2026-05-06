use crate::executor::adapters::outbound::FileSystemWriter;
use crate::shared::models::{BodiesJson, FuzzerConfigArtifact};
use anyhow::Result;

pub async fn write_bodies(bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
    let contract = &bodies.meta.contract;
    let path = format!(".fuzzming/{}/{}.bodies.json", contract, contract);
    let content = serde_json::to_string_pretty(bodies)?;
    writer.write_file(&path, &content).await
}

pub async fn write_config_json(
    config: &FuzzerConfigArtifact,
    contract: &str,
    writer: &FileSystemWriter,
) -> Result<()> {
    let path = format!(".fuzzming/{}/{}.config.json", contract, contract);
    let content = serde_json::to_string_pretty(config)?;
    writer.write_file(&path, &content).await
}
