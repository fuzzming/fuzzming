use crate::llm::ports::{LlmGenerationPort, LlmGenerationRequest, LlmGenerationResponse};
use crate::llm::use_cases::apply_patch::{apply_bodies_patch, apply_config_patch};
use crate::llm::use_cases::assemble_prompt::assemble_prompt;
use crate::shared::models::AssembledPrompt;
use crate::shared::models::ContractContext;
use crate::shared::ports::LlmEnginePort;
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::llm_signal::{LlmSignal, LlmStatus};
use anyhow::Result;
use async_trait::async_trait;

pub struct Llm {
    pub gateway: Box<dyn LlmGenerationPort>,
}

impl Llm {
    pub fn new(gateway: Box<dyn LlmGenerationPort>) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl LlmEnginePort for Llm {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal> {
        let prompt: AssembledPrompt = assemble_prompt(
            signal.round,
            ContractContext {
                source_code: signal.source_code.clone(),
            },
            signal.fuzz_output.clone(),
            signal.coverage_context.clone(),
        )?;

        let request = LlmGenerationRequest {
            round: signal.round,
            source_code: signal.source_code.clone(),
            prompt,
            existing_bodies: signal.existing_bodies.clone(),
            existing_foundry_config: signal.existing_foundry_config.clone(),
        };

        let response = self.gateway.generate(request).await?;

        let (bodies, foundry_config) = match response {
            LlmGenerationResponse::Full {
                bodies,
                foundry_config,
            } => (bodies, foundry_config),
            LlmGenerationResponse::Patch {
                bodies_updates,
                foundry_config_updates,
            } => {
                let existing_bodies = signal.existing_bodies.ok_or_else(|| {
                    anyhow::anyhow!("received Patch response on round 1 — no existing bodies")
                })?;
                let existing_config = signal.existing_foundry_config.ok_or_else(|| {
                    anyhow::anyhow!("received Patch response on round 1 — no existing config")
                })?;

                let bodies = apply_bodies_patch(existing_bodies, &bodies_updates)?;
                let foundry_config = apply_config_patch(existing_config, &foundry_config_updates)?;
                (bodies, foundry_config)
            }
        };

        Ok(LlmSignal {
            status: LlmStatus::Done,
            bodies: Some(bodies),
            foundry_config: Some(foundry_config),
            reason: None,
        })
    }
}
