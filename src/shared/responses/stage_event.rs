use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageKind {
    Llm,
    Executor,
    Fuzzer,
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
