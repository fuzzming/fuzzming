use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::llm_signal::LlmSignal;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait GeneratorRunPort: Send + Sync {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal>;
}
