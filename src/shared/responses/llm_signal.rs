use crate::shared::models::{BodiesJson, FoundryConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmStatus {
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSignal {
    pub status: LlmStatus,
    pub bodies: Option<BodiesJson>,
    pub foundry_config: Option<FoundryConfig>,
    pub reason: Option<String>,
}
