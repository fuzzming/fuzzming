use anyhow::Result;
use async_trait::async_trait;

use crate::interfaces::signals::{LlmSignal, RoundSignal};
use crate::llm::ports::{ExecutorPort, LlmGateway, LlmReaderPort};
use crate::orchestrator::ports::LlmEnginePort;

pub struct LlmEngine {
    pub reader: Box<dyn LlmReaderPort>,
    pub executor: Box<dyn ExecutorPort>,
    pub gateway: Box<dyn LlmGateway>,
}

impl LlmEngine {
    pub fn new(
        reader: Box<dyn LlmReaderPort>,
        executor: Box<dyn ExecutorPort>,
        gateway: Box<dyn LlmGateway>,
    ) -> Self {
        Self {
            reader,
            executor,
            gateway,
        }
    }
}

#[async_trait]
impl LlmEnginePort for LlmEngine {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal> {
        todo!()
    }
}
