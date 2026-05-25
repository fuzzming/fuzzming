use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::shared::models::{BugInfo, SessionConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub rounds_remaining: u32,
    pub current_round: u32,
    pub config: SessionConfig,
    /// All bugs accumulated across every round, keyed by contract name.
    pub found_bugs: HashMap<String, Vec<BugInfo>>,
    /// Number of consecutive rounds with 100% coverage, keyed by contract name.
    pub full_coverage_streak: HashMap<String, u32>,
}
