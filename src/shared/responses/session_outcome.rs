use crate::shared::responses::fuzz_report::FuzzPaths;
use serde::{Deserialize, Serialize};

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
    pub paths: FuzzPaths,
}
