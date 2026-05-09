use serde::{Deserialize, Serialize};

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
