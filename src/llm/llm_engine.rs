use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::signals::{RoundSignal, LlmSignal};
use crate::interfaces::ports::LlmEnginePort;
use crate::llm::ports::LlmGateway;

pub struct LlmEngine {
    pub gateway: Box<dyn LlmGateway>,
}

impl LlmEngine {
    pub fn new(gateway: Box<dyn LlmGateway>) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl LlmEnginePort for LlmEngine {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal> {
        todo!()
    }
}
