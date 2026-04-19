use crate::shared::{
    ports::{FuzzerEnginePort, LlmEnginePort},
    requests::round_signal::RoundSignal,
    responses::{fuzz_report::FuzzReport, llm_signal::LlmSignal},
};
use anyhow::Result;

pub async fn run_round(
    signal: RoundSignal,
    llm_engine: &dyn LlmEnginePort,
    fuzzer_engine: &dyn FuzzerEnginePort,
) -> Result<(LlmSignal, FuzzReport)> {
    todo!()
}
