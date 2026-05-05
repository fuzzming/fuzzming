use anyhow::{anyhow, Result};

use crate::generator::domain::generation_response::GenerationResponse;
use crate::shared::{
    models::{ExecutorInput, FuzzerConfigArtifact},
    ports::{ExecutorPort, LlmEnginePort},
    requests::round_signal::RoundSignal,
    responses::llm_signal::LlmSignal,
};

/// Runs the LLM and Executor for a single contract within a round.
/// The fuzzer is intentionally excluded — it is called once for all contracts
/// after all LLM + Executor calls complete (see run_session).
pub async fn run_round(
    signal: RoundSignal,
    llm_engine: &dyn LlmEnginePort,
    executor: &dyn ExecutorPort,
) -> Result<LlmSignal> {
    let mut llm_signal = llm_engine.run(signal.clone()).await?;

    let result = llm_signal
        .result
        .as_mut()
        .ok_or_else(|| anyhow!("LLM returned no result for contract '{}'", signal.contract_name))?;

    // Option B: deterministically strip confirmed-broken invariants from Full responses
    // so forge never re-runs them and the LLM never sees stale failure signal for them.
    if let GenerationResponse::Full { ref mut bodies, .. } = result.response {
        for bug in &signal.confirmed_bugs {
            bodies.invariant_test.invariants.shift_remove(&bug.invariant_name);
        }
    }

    let executor_input = build_executor_input(&result.response, &signal)?;
    executor.execute(executor_input).await?;

    Ok(llm_signal)
}

fn build_executor_input(response: &GenerationResponse, signal: &RoundSignal) -> Result<ExecutorInput> {
    match response {
        GenerationResponse::Full { bodies, foundry_config } => Ok(ExecutorInput::Full {
            bodies: bodies.clone(),
            fuzzer_config: FuzzerConfigArtifact::Foundry(foundry_config.clone()),
        }),

        GenerationResponse::Patch { bodies_updates, foundry_config_updates } => {
            let existing_bodies = signal
                .existing_bodies
                .clone()
                .ok_or_else(|| anyhow!("patch response with no existing bodies for '{}'", signal.contract_name))?;
            let existing_config = signal
                .existing_foundry_config
                .clone()
                .ok_or_else(|| anyhow!("patch response with no existing foundry config for '{}'", signal.contract_name))?;
            Ok(ExecutorInput::Patch {
                existing_bodies,
                bodies_updates: bodies_updates.clone(),
                existing_config: FuzzerConfigArtifact::Foundry(existing_config),
                config_updates: foundry_config_updates.clone(),
            })
        }
    }
}
