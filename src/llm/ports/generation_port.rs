use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::models::{AssembledPrompt, BodiesJson, FoundryConfig};

#[derive(Debug, Clone)]
pub struct LlmGenerationRequest {
    pub round: u32,
    pub source_code: String,
    pub prompt: AssembledPrompt,
    pub existing_bodies: Option<BodiesJson>,
    pub existing_foundry_config: Option<FoundryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum LlmGenerationResponse {
    Full {
        bodies: BodiesJson,
        foundry_config: FoundryConfig,
    },
    Patch {
        bodies_updates: Vec<JsonBlockUpdate>,
        foundry_config_updates: Vec<JsonBlockUpdate>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonBlockUpdate {
    /// Dot path of the block to replace in the previous JSON artifact.
    /// Example: "handler.functions.invariant_deposit".
    pub path: String,
    pub value: Value,
    pub reason: String,
}

#[async_trait]
pub trait LlmGenerationPort: Send + Sync {
    async fn generate(&self, request: LlmGenerationRequest) -> Result<LlmGenerationResponse>;
}
