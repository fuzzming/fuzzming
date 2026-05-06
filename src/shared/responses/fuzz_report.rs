use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::shared::models::BugInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FuzzOutcome {
    Bug,
    Pass,
    FullCoverage,
    /// Forge could not compile the generated files. The LLM should repair them.
    CompileError,
    DevTestFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzReport {
    pub outcome: FuzzOutcome,
    /// All failing invariants found in this forge run.
    /// Populated only when outcome == Bug; empty otherwise.
    pub bugs: Vec<BugInfo>,
    /// Path to the lcov.info file written by `forge coverage`.
    /// Set only when outcome is Pass or FullCoverage; None otherwise.
    pub lcov_path: Option<PathBuf>,
}
