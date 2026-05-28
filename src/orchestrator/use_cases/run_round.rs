use anyhow::{anyhow, Result};
use tracing::info;

use crate::generator::domain::generation_response::GenerationResponse;
use crate::shared::{
    models::{ExecutorInput, FuzzerConfigArtifact},
    ports::{ExecutorPort, LlmEnginePort, ReporterPort},
    requests::round_signal::RoundSignal,
    responses::{
        llm_signal::{LlmSignal, LlmStatus},
        stage_event::{StageEvent, StageKind, StageStatus},
    },
};

/// Runs the LLM and Executor for a single contract within a round.
/// The fuzzer is intentionally excluded — it is called once for all contracts
/// after all LLM + Executor calls complete (see run_session).
pub async fn run_round(
    signal: RoundSignal,
    llm_engine: &dyn LlmEnginePort,
    executor: &dyn ExecutorPort,
    reporter: &dyn ReporterPort,
) -> Result<LlmSignal> {
    reporter
        .emit_stage_event(StageEvent {
            contract_name: Some(signal.contract_name.clone()),
            round: signal.round,
            stage: StageKind::Llm,
            status: StageStatus::Started,
            fuzzer_summary: None,
        })
        .await?;
    info!(contract = %signal.contract_name, round = signal.round, "LLM started");
    let mut llm_signal = llm_engine.run(signal.clone()).await?;

    // Emit the correct status so the terminal handler can close the spinner.
    let llm_stage_status = if matches!(llm_signal.status, LlmStatus::Failed) {
        StageStatus::Failed
    } else {
        StageStatus::Finished
    };
    reporter
        .emit_stage_event(StageEvent {
            contract_name: Some(signal.contract_name.clone()),
            round: signal.round,
            stage: StageKind::Llm,
            status: llm_stage_status,
            fuzzer_summary: None,
        })
        .await?;

    // LLM failure — skip the executor; the error will be injected into the next round.
    if matches!(llm_signal.status, LlmStatus::Failed) {
        let msg = format!(
            "LLM call failed:\n{}",
            llm_signal.reason.as_deref().unwrap_or("unknown error")
        );
        reporter
            .emit_compile_error(signal.round, &msg)
            .await?;
        return Ok(llm_signal);
    }

    let result = llm_signal.result.as_mut().ok_or_else(|| {
        anyhow!(
            "LLM returned no result for contract '{}'",
            signal.contract_name
        )
    })?;

    // Strip confirmed-broken invariants to avoid reruns and stale failure signals.
    if let GenerationResponse::Full { ref mut bodies, .. } = result.response {
        let stripped: Vec<&str> = signal
            .confirmed_bugs
            .iter()
            .filter(|b| {
                bodies
                    .invariant_test
                    .invariants
                    .shift_remove(&b.invariant_name)
                    .is_some()
            })
            .map(|b| b.invariant_name.as_str())
            .collect();
        if !stripped.is_empty() {
            info!(contract = %signal.contract_name, stripped = ?stripped, "stripped confirmed invariants");
        }
    }

    info!(contract = %signal.contract_name, round = signal.round, "LLM done — executor writing files");
    reporter
        .emit_stage_event(StageEvent {
            contract_name: Some(signal.contract_name.clone()),
            round: signal.round,
            stage: StageKind::Executor,
            status: StageStatus::Started,
            fuzzer_summary: None,
        })
        .await?;
    let executor_input = build_executor_input(&result.response, &signal)?;
    executor.execute(executor_input).await?;
    info!(contract = %signal.contract_name, round = signal.round, "executor done");

    reporter
        .emit_stage_event(StageEvent {
            contract_name: Some(signal.contract_name.clone()),
            round: signal.round,
            stage: StageKind::Executor,
            status: StageStatus::Finished,
            fuzzer_summary: None,
        })
        .await?;

    Ok(llm_signal)
}

fn build_executor_input(
    response: &GenerationResponse,
    signal: &RoundSignal,
) -> Result<ExecutorInput> {
    match response {
        GenerationResponse::Full {
            bodies,
            foundry_config,
        } => Ok(ExecutorInput::Full {
            bodies: *bodies.clone(),
            fuzzer_config: FuzzerConfigArtifact::Foundry(*foundry_config.clone()),
            source_pragma: signal.source_pragma.clone(),
        }),

        GenerationResponse::Patch {
            bodies_updates,
            foundry_config_updates,
        } => {
            let existing_bodies = signal.existing_bodies.clone().ok_or_else(|| {
                anyhow!(
                    "patch response with no existing bodies for '{}'",
                    signal.contract_name
                )
            })?;
            let existing_config = signal.existing_foundry_config.clone().ok_or_else(|| {
                anyhow!(
                    "patch response with no existing foundry config for '{}'",
                    signal.contract_name
                )
            })?;
            Ok(ExecutorInput::Patch {
                existing_bodies,
                bodies_updates: bodies_updates.clone(),
                existing_config: FuzzerConfigArtifact::Foundry(existing_config),
                config_updates: foundry_config_updates.clone(),
                source_pragma: signal.source_pragma.clone(),
            })
        }
    }
}
