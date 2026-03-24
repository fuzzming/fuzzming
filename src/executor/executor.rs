use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::artifacts::{InvariantSet, FoundryConfig};
use crate::llm::ports::ExecutorPort;
use crate::executor::infrastructure::FileSystemWriter;

pub struct Executor {
    pub writer: FileSystemWriter,
}

impl Executor {
    pub fn new(writer: FileSystemWriter) -> Self {
        Self { writer }
    }
}

#[async_trait]
impl ExecutorPort for Executor {
    async fn write_invariants(&self, set: InvariantSet) -> Result<()> {
        todo!()
    }

    async fn write_foundry_config(&self, config: FoundryConfig) -> Result<()> {
        todo!()
    }
}
