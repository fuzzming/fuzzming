use crate::llm::ports::outbound::{LlmGenerationPort, LlmGenerationRequest};
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

        Ok(LlmSignal {
            status: LlmStatus::Done,
            result: Some(response),
            reason: None,
        })
    }
}
