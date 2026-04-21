use anyhow::Result;
use async_trait::async_trait;

use crate::llm::domain::llm_generation_response::LlmGenerationResult;
use crate::shared::models::{AssembledPrompt, BodiesJson, FoundryConfig};

#[derive(Debug, Clone)]
pub struct LlmGenerationRequest {
    pub round: u32,
    pub source_code: String,
    pub prompt: AssembledPrompt,
    pub existing_bodies: Option<BodiesJson>,
    pub existing_foundry_config: Option<FoundryConfig>,
}

#[async_trait]
pub trait LlmGenerationPort: Send + Sync {
    async fn generate(&self, request: LlmGenerationRequest) -> Result<LlmGenerationResult>;
}
