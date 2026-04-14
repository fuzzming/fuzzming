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
    /// Current contents of foundry.toml, read by Reader and forwarded here so the
    /// Executor can patch only the managed sections without ever reading the file itself.
    /// `None` when foundry.toml does not exist yet.
    pub current_toml: Option<String>,
}
