use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use crate::executor::adapters::outbound::FileSystemWriter;
use crate::executor::ports::inbound::ExecutorRunPort;
use crate::executor::ports::outbound::{CodeGeneratorPort, ConfigWriterPort};
use crate::executor::use_cases::apply_patch::apply_patches;
use crate::shared::models::{ExecutorInput, FuzzerConfigArtifact};

use super::write_bodies::{write_bodies, write_config_json};

pub struct ExecuteUseCase {
    writer: FileSystemWriter,
    generator: Arc<dyn CodeGeneratorPort>,
    config_writer: Arc<dyn ConfigWriterPort>,
}

impl ExecuteUseCase {
    pub fn new(
        writer: FileSystemWriter,
        generator: Arc<dyn CodeGeneratorPort>,
        config_writer: Arc<dyn ConfigWriterPort>,
    ) -> Self {
        Self {
            writer,
            generator,
            config_writer,
        }
    }
}

#[async_trait]
impl ExecutorRunPort for ExecuteUseCase {
    async fn execute(&self, input: ExecutorInput) -> Result<()> {
        let (bodies, fuzzer_config) = resolve_input(input)?;

        write_bodies(&bodies, &self.writer).await?;
        write_config_json(&fuzzer_config, &bodies.meta.contract, &self.writer).await?;
        self.generator.generate(&bodies, &self.writer).await?;
        self.config_writer
            .write(&fuzzer_config, &self.writer)
            .await?;
        Ok(())
    }
}

/// Resolve an `ExecutorInput` into concrete `(BodiesJson, FuzzerConfigArtifact)` by either
/// passing through the full artifacts (round 1) or applying patch operations (round N).
fn resolve_input(
    input: ExecutorInput,
) -> Result<(crate::shared::models::BodiesJson, FuzzerConfigArtifact)> {
    match input {
        ExecutorInput::Full {
            bodies,
            fuzzer_config,
        } => Ok((bodies, fuzzer_config)),

        ExecutorInput::Patch {
            existing_bodies,
            bodies_updates,
            existing_config,
            config_updates,
        } => {
            let patched_bodies = apply_patches(existing_bodies, &bodies_updates)?;
            // Unwrap the enum so patch paths apply to FoundryConfig fields.
            let patched_config = match existing_config {
                FuzzerConfigArtifact::Foundry(inner) => {
                    FuzzerConfigArtifact::Foundry(apply_patches(inner, &config_updates)?)
                }
            };
            Ok((patched_bodies, patched_config))
        }
    }
}
