use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::signals::{RoundSignal, LlmSignal};
use crate::interfaces::ports::LlmEnginePort;
use crate::llm::ports::LlmGenerationPort;

pub struct Llm {
    pub gateway: Box<dyn LlmGenerationPort>,
}

impl Llm {
    pub fn new(gateway: Box<dyn LlmGenerationPort>) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl LlmEnginePort for Llm {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal> {
        todo!()
    }
}
