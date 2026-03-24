use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FuzzOutcome {
    Bug,
    Pass,
    FullCoverage,
    DevTestFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzPaths {
    pub fuzz_output: String,
    pub lcov: Option<String>,
    pub invariant_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzReport {
    pub outcome: FuzzOutcome,
    pub paths: FuzzPaths,
}
