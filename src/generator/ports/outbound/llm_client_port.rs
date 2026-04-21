use crate::generator::domain::generation_response::GenerationUsage;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait LlmClientPort: Send + Sync {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<(String, Option<GenerationUsage>)>;
}
