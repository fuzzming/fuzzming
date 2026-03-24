use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::artifacts::AssembledPrompt;

#[async_trait]
pub trait LlmGateway: Send + Sync {
    async fn call(&self, prompt: AssembledPrompt) -> Result<String>;
}
