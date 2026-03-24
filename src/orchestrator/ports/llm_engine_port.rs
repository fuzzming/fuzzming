use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::signals::{RoundSignal, LlmSignal};

#[async_trait]
pub trait LlmEnginePort: Send + Sync {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal>;
}
