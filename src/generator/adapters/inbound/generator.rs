use anyhow::Result;
use async_trait::async_trait;

use crate::generator::ports::inbound::GeneratorRunPort;
use crate::shared::ports::LlmEnginePort;
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::llm_signal::LlmSignal;

pub struct Generator {
    use_case: Box<dyn GeneratorRunPort>,
}

impl Generator {
    pub fn new(use_case: Box<dyn GeneratorRunPort>) -> Self {
        Self { use_case }
    }
}

#[async_trait]
impl LlmEnginePort for Generator {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal> {
        self.use_case.run(signal).await
    }
}
