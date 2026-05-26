use serde::{Deserialize, Serialize};

use crate::shared::models::BugInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminationReason {
    Bug,
    Exhausted,
    FullCoverage,
    DevTestFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOutcome {
    pub reason: TerminationReason,
    pub contract_name: String,
    pub rounds_completed: u32,
    pub bugs: Vec<BugInfo>,
    pub coverage_snapshots: Vec<String>,
}
