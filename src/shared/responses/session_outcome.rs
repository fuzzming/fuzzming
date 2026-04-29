use serde::{Deserialize, Serialize};

use crate::shared::models::ReportArtifacts;

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
    pub artifacts: ReportArtifacts,
}
