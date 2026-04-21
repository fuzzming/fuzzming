use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::de::DeserializeOwned;

use super::prompt_builder::{
    build_round_n_prompt, build_round_one_analysis_prompt, build_round_one_bodies_prompt,
    build_round_one_config_prompt, system_prompt_from_request,
};
use super::response_parser::{
    build_parse_repair_prompt, extract_json_payload, parse_generation_response,
};
use super::stages::{AnalysisStage, BodiesStage, ConfigStage};
use crate::llm::domain::llm_generation_response::{
    LlmGenerationResponse, LlmGenerationResult, LlmUsage,
};
use crate::llm::ports::outbound::{LlmClientPort, LlmGenerationPort, LlmGenerationRequest};

const MAX_ATTEMPTS: usize = 2;

pub struct LiteLlmGenerationAdapter {
    model: String,
    api_key: String,
    client: Box<dyn LlmClientPort>,
}

impl LiteLlmGenerationAdapter {
    pub fn new(model: impl Into<String>, api_key: impl Into<String>, client: Box<dyn LlmClientPort>) -> Self {
        Self {
            model: model.into(),
            api_key: api_key.into(),
            client,
        }
    }

    fn set_api_key(&self) {
        if let Some(prefix) = self.model.split('/').next() {
            let env_var = format!("{}_API_KEY", prefix.to_uppercase());
            std::env::set_var(env_var, &self.api_key);
        }
    }

    fn merge_usage(total: &mut LlmUsage, usage: Option<LlmUsage>) {
        if let Some(usage) = usage {
            total.calls = total.calls.saturating_add(usage.calls);
            total.prompt_tokens = total.prompt_tokens.saturating_add(usage.prompt_tokens);
            total.completion_tokens = total
                .completion_tokens
                .saturating_add(usage.completion_tokens);
            total.total_tokens = total.total_tokens.saturating_add(usage.total_tokens);
            total.cached_prompt_tokens = total
                .cached_prompt_tokens
                .saturating_add(usage.cached_prompt_tokens);
            total.reasoning_tokens = total.reasoning_tokens.saturating_add(usage.reasoning_tokens);
            total.thinking_tokens = total.thinking_tokens.saturating_add(usage.thinking_tokens);
        }
    }

    async fn request_json<T>(
        &self,
        system_prompt: &str,
        initial_prompt: String,
        stage_name: &str,
        schema_hint: &str,
        usage_total: &mut LlmUsage,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut user_prompt = initial_prompt;
        let mut last_error = String::new();

        for attempt in 1..=MAX_ATTEMPTS {
            let (content, usage) = self.client.complete(system_prompt, &user_prompt).await?;
            Self::merge_usage(usage_total, usage);
            let payload = extract_json_payload(&content)?;

            match serde_json::from_str::<T>(&payload)
                .with_context(|| format!("failed to parse {stage_name} payload: {payload}"))
            {
                Ok(parsed) => return Ok(parsed),
                Err(err) => {
                    last_error = err.to_string();
                    if attempt == MAX_ATTEMPTS {
                        break;
                    }
                    user_prompt =
                        build_parse_repair_prompt(stage_name, schema_hint, &payload, &last_error);
                }
            }
        }

        bail!("{stage_name} failed after {} attempts: {}", MAX_ATTEMPTS, last_error)
    }

    async fn generate_round_one(
        &self,
        request: &LlmGenerationRequest,
    ) -> Result<LlmGenerationResult> {
        let system_prompt = system_prompt_from_request(request);
        let mut usage = LlmUsage::default();

        let analysis: AnalysisStage = self
            .request_json(
                &system_prompt,
                build_round_one_analysis_prompt(),
                "analysis",
                "vulnerability_analysis, handler_logic_pseudocode, invariant_mathematical_proofs",
                &mut usage,
            )
            .await?;

        let bodies_stage: BodiesStage = self
            .request_json(
                &system_prompt,
                build_round_one_bodies_prompt(&analysis)?,
                "bodies",
                "bodies object with valid Solidity syntax",
                &mut usage,
            )
            .await?;

        let config_stage: ConfigStage = self
            .request_json(
                &system_prompt,
                build_round_one_config_prompt(&analysis, &bodies_stage.bodies)?,
                "config",
                "foundry_config mapping to handler functions",
                &mut usage,
            )
            .await?;

        Ok(LlmGenerationResult {
            response: LlmGenerationResponse::Full {
                bodies: bodies_stage.bodies,
                foundry_config: config_stage.foundry_config,
            },
            usage,
        })
    }

    async fn generate_round_n(
        &self,
        request: &LlmGenerationRequest,
    ) -> Result<LlmGenerationResult> {
        let system_prompt = system_prompt_from_request(request);
        let mut user_prompt = build_round_n_prompt(request)?;
        let mut last_error = String::new();
        let mut usage = LlmUsage::default();

        for attempt in 1..=MAX_ATTEMPTS {
            let (content, call_usage) = self.client.complete(&system_prompt, &user_prompt).await?;
            Self::merge_usage(&mut usage, call_usage);
            let payload = extract_json_payload(&content)?;

            match parse_generation_response(&payload) {
                Ok(parsed) => {
                    return Ok(LlmGenerationResult {
                        response: parsed,
                        usage,
                    })
                }
                Err(err) => {
                    last_error = err.to_string();
                    if attempt == MAX_ATTEMPTS {
                        break;
                    }
                    user_prompt = build_parse_repair_prompt(
                        "round generation",
                        "mode + canonical full/patch fields",
                        &payload,
                        &last_error,
                    );
                }
            }
        }

        bail!(
            "model returned invalid structured output after {} attempts: {}",
            MAX_ATTEMPTS,
            last_error
        )
    }
}

#[async_trait]
impl LlmGenerationPort for LiteLlmGenerationAdapter {
    async fn generate(&self, request: LlmGenerationRequest) -> Result<LlmGenerationResult> {
        self.set_api_key();

        if request.round == 1 {
            self.generate_round_one(&request).await
        } else {
            self.generate_round_n(&request).await
        }
    }
}
