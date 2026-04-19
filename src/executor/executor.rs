use std::sync::Arc;

use crate::executor::infrastructure::FileSystemWriter;
use crate::executor::ports::{CodeGeneratorPort, ConfigWriterPort};
use crate::executor::use_cases::write_bodies::write_bodies;
use crate::shared::models::ExecutorInput;
use crate::shared::ports::ExecutorPort;
use anyhow::Result;
use async_trait::async_trait;

pub struct Executor {
    writer: FileSystemWriter,
    generator: Arc<dyn CodeGeneratorPort>,
    config_writer: Arc<dyn ConfigWriterPort>,
}

impl Executor {
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
impl ExecutorPort for Executor {
    async fn execute(&self, input: ExecutorInput) -> Result<()> {
        write_bodies(&input.bodies, &self.writer).await?;
        self.generator.generate(&input.bodies, &self.writer).await?;
        self.config_writer
            .write(&input.fuzzer_config, &self.writer)
            .await?;
        Ok(())
    }
}
