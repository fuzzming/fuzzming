use serde::{Deserialize, Serialize};
use crate::interfaces::signals::fuzz_report::FuzzPaths;

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
