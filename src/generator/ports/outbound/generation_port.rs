use anyhow::Result;
use async_trait::async_trait;

use crate::generator::domain::generation_response::GenerationResult;
use crate::shared::models::{AssembledPrompt, BodiesJson, FoundryConfig};

#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub round: u32,
    pub source_code: String,
    pub prompt: AssembledPrompt,
    pub existing_bodies: Option<BodiesJson>,
    pub existing_foundry_config: Option<FoundryConfig>,
}

#[async_trait]
pub trait GenerationPort: Send + Sync {
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResult>;
}
