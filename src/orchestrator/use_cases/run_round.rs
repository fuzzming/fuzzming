use anyhow::Result;
use crate::interfaces::signals::{RoundSignal, FuzzReport, LlmSignal};
use crate::orchestrator::ports::{LlmEnginePort, FuzzerEnginePort};

pub async fn run_round(
    signal: RoundSignal,
    llm_engine: &dyn LlmEnginePort,
    fuzzer_engine: &dyn FuzzerEnginePort,
) -> Result<(LlmSignal, FuzzReport)> {
    todo!()
}
