use std::path::PathBuf;

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
    /// Path to the lcov.info file written by `forge coverage`.
    /// Set only when outcome is Pass or FullCoverage; None otherwise.
    pub lcov_path: Option<PathBuf>,
}
