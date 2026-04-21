use anyhow::Result;
use async_trait::async_trait;

use crate::llm::ports::inbound::LlmRunPort;
use crate::shared::ports::LlmEnginePort;
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::llm_signal::LlmSignal;

pub struct Llm {
    use_case: Box<dyn LlmRunPort>,
}

impl Llm {
    pub fn new(use_case: Box<dyn LlmRunPort>) -> Self {
        Self { use_case }
    }
}

#[async_trait]
impl LlmEnginePort for Llm {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal> {
        self.use_case.run(signal).await
    }
}
