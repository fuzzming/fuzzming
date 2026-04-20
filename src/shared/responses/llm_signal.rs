use crate::llm::domain::llm_generation_response::LlmGenerationResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmStatus {
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSignal {
    pub status: LlmStatus,
    pub result: Option<LlmGenerationResult>,
    pub reason: Option<String>,
}
