use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageKind {
    Llm,
    Executor,
    Fuzzer,
    /// Emitted when a single contract's session terminates (all rounds done).
    ContractDone,
    /// Emitted once when the entire multi-contract session ends.
    SessionDone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageStatus {
    Started,
    Finished,
    Failed,
}

/// Attached to Fuzzer::Finished so the UI can show per-round numbers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzerRoundSummary {
    pub bugs: usize,
    pub passed: usize,
    pub compile_errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageEvent {
    pub contract_name: Option<String>,
    pub round: u32,
    pub stage: StageKind,
    pub status: StageStatus,
    /// Populated only on Fuzzer::Finished.
    #[serde(default)]
    pub fuzzer_summary: Option<FuzzerRoundSummary>,
}
