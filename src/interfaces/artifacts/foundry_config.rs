use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundryConfig {
    pub depth: u32,
    pub runs: u32,
    pub seed: String,
    pub max_test_rejects: u32,
    pub dictionary_weight: u32,
    pub call_sequence_weights: HashMap<String, f64>,
}
