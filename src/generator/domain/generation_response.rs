use serde::{Deserialize, Serialize};

use crate::shared::models::{BodiesJson, FoundryConfig, GenerationUsage, JsonBlockUpdate};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum GenerationResponse {
    Full {
        bodies: Box<BodiesJson>,
        foundry_config: Box<FoundryConfig>,
    },
    Patch {
        bodies_updates: Vec<JsonBlockUpdate>,
        foundry_config_updates: Vec<JsonBlockUpdate>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub response: GenerationResponse,
    pub usage: GenerationUsage,
}
