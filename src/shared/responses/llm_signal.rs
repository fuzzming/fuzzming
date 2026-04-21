use crate::generator::domain::generation_response::GenerationResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmStatus {
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSignal {
    pub status: LlmStatus,
    pub result: Option<GenerationResult>,
    pub reason: Option<String>,
}
