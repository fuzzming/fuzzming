use anyhow::Result;
use async_trait::async_trait;

use crate::generator::ports::inbound::GeneratorRunPort;
use crate::generator::ports::outbound::{GenerationPort, GenerationRequest};
use crate::shared::models::ContractContext;
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::llm_signal::{LlmSignal, LlmStatus};

use super::assemble_prompt::assemble_prompt;

pub struct GeneratorRunUseCase {
    gateway: Box<dyn GenerationPort>,
}

impl GeneratorRunUseCase {
    pub fn new(gateway: Box<dyn GenerationPort>) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl GeneratorRunPort for GeneratorRunUseCase {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal> {
        let prompt = assemble_prompt(
            signal.round,
            ContractContext {
                source_code: signal.source_code.clone(),
            },
            signal.fuzz_output.clone(),
            signal.coverage_context.clone(),
            signal.confirmed_bugs.clone(),
            signal.security_analysis.clone(),
        )?;

        let request = GenerationRequest {
            round: signal.round,
            contract_name: signal.contract_name.clone(),
            contract_path: signal.contract_path.clone(),
            source_code: signal.source_code.clone(),
            prompt,
            existing_bodies: signal.existing_bodies.clone(),
            existing_foundry_config: signal.existing_foundry_config.clone(),
        };

        let response = match self.gateway.generate(request).await {
            Ok(r) => r,
            Err(e) => {
                return Ok(LlmSignal {
                    status: LlmStatus::Failed,
                    result: None,
                    reason: Some(e.to_string()),
                });
            }
        };

        Ok(LlmSignal {
            status: LlmStatus::Done,
            result: Some(response),
            reason: None,
        })
    }
}
