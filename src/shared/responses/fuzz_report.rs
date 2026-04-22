use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FuzzOutcome {
    Bug,
    Pass,
    FullCoverage,
    DevTestFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzReport {
    pub outcome: FuzzOutcome,
}
