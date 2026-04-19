use crate::shared::{requests::round_signal::RoundSignal, responses::llm_signal::LlmSignal};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait LlmEnginePort: Send + Sync {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal>;
}
