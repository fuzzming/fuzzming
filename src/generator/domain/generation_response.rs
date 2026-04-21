use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::models::{BodiesJson, FoundryConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum GenerationResponse {
    Full {
        bodies: BodiesJson,
        foundry_config: FoundryConfig,
    },
    Patch {
        bodies_updates: Vec<JsonBlockUpdate>,
        foundry_config_updates: Vec<JsonBlockUpdate>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerationUsage {
    pub calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub reasoning_tokens: u64,
    pub thinking_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub response: GenerationResponse,
    pub usage: GenerationUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonBlockUpdate {
    #[serde(default)]
    pub op: JsonPatchOp,
    /// Dot path of the block to replace in the previous JSON artifact.
    /// Example: "handler.functions.invariant_deposit".
    pub path: String,
    pub value: Value,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum JsonPatchOp {
    Add,
    #[serde(alias = "modify")]
    #[default]
    Replace,
    Remove,
}
