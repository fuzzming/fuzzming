use crate::executor::adapters::outbound::FileSystemWriter;
use crate::shared::models::{BodiesJson, FuzzerConfigArtifact};
use anyhow::Result;

pub async fn write_bodies(bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
    let contract = &bodies.meta.contract;
    let path = format!(".fuzzming/{contract}/{contract}.bodies.json");
    let content = serde_json::to_string_pretty(bodies)?;
    writer.write_file(&path, &content).await
}

pub async fn write_config_json(
    config: &FuzzerConfigArtifact,
    contract: &str,
    writer: &FileSystemWriter,
) -> Result<()> {
    let path = format!(".fuzzming/{contract}/{contract}.config.json");
    let content = serde_json::to_string_pretty(config)?;
    writer.write_file(&path, &content).await
}
