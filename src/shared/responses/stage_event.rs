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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageEvent {
    pub contract_name: Option<String>,
    pub round: u32,
    pub stage: StageKind,
    pub status: StageStatus,
}
