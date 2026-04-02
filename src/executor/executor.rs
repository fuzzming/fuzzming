use crate::executor::infrastructure::FileSystemWriter;
use crate::executor::writers::bodies_writer::write_bodies;
use crate::executor::writers::foundry_config_writer::write_foundry_config;
use crate::executor::writers::solidity_generator::{generate_handler, generate_invariant_test};
use crate::interfaces::artifacts::{BodiesJson, FoundryConfig};
use crate::llm::ports::ExecutorPort;
use anyhow::Result;
use async_trait::async_trait;

pub struct Executor {
    writer: FileSystemWriter,
}

impl Executor {
    pub fn new(writer: FileSystemWriter) -> Self {
        Self { writer }
    }
}

#[async_trait]
impl ExecutorPort for Executor {
    async fn write_bodies(&self, bodies: BodiesJson) -> Result<()> {
        write_bodies(&bodies, &self.writer).await?;
        generate_handler(&bodies, &self.writer).await?;
        generate_invariant_test(&bodies, &self.writer).await?;
        Ok(())
    }

    async fn write_foundry_config(&self, config: FoundryConfig) -> Result<()> {
        write_foundry_config(&config, &self.writer).await
    }
}
