use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::de::DeserializeOwned;

use super::prompt_builder::{
    build_body_schema, build_round_n_analysis_prompt, build_round_n_patch_prompt,
    build_round_one_analysis_prompt, build_round_one_bodies_prompt, build_round_one_config_prompt,
    system_prompt_from_request, user_prompt_from_request,
};
use super::response_parser::{
    build_parse_repair_prompt, extract_json_payload, parse_generation_response,
};
use super::stages::{AnalysisStage, BodiesStage, ConfigStage, PatchAnalysisStage};
use crate::generator::domain::generation_response::{
    GenerationResponse, GenerationResult, GenerationUsage,
};
use crate::generator::ports::outbound::{GenerationPort, GenerationRequest, LlmClientPort};
use crate::shared::models::BodiesJson;

const MAX_ATTEMPTS: usize = 2;

pub struct LiteLlmGenerationAdapter {
    model: String,
    api_key: String,
    client: Box<dyn LlmClientPort>,
}

impl LiteLlmGenerationAdapter {
    pub fn new(
        model: impl Into<String>,
        api_key: impl Into<String>,
        client: Box<dyn LlmClientPort>,
    ) -> Self {
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

    fn merge_usage(total: &mut GenerationUsage, usage: Option<GenerationUsage>) {
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
            total.reasoning_tokens = total
                .reasoning_tokens
                .saturating_add(usage.reasoning_tokens);
            total.thinking_tokens = total.thinking_tokens.saturating_add(usage.thinking_tokens);
        }
    }

    async fn request_json<T>(
        &self,
        system_prompt: &str,
        initial_prompt: String,
        stage_name: &str,
        schema_hint: &str,
        usage_total: &mut GenerationUsage,
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

        bail!(
            "{stage_name} failed after {} attempts: {}",
            MAX_ATTEMPTS,
            last_error
        )
    }

    async fn request_generation_response(
        &self,
        system_prompt: &str,
        initial_prompt: String,
        stage_name: &str,
        schema_hint: &str,
        usage_total: &mut GenerationUsage,
    ) -> Result<GenerationResponse> {
        let mut user_prompt = initial_prompt;
        let mut last_error = String::new();

        for attempt in 1..=MAX_ATTEMPTS {
            let (content, usage) = self.client.complete(system_prompt, &user_prompt).await?;
            Self::merge_usage(usage_total, usage);
            let payload = extract_json_payload(&content)?;

            match parse_generation_response(&payload) {
                Ok(parsed) => match validate_generation_response(&parsed) {
                    Ok(()) => return Ok(parsed),
                    Err(err) => {
                        last_error = err.to_string();
                        if attempt == MAX_ATTEMPTS {
                            break;
                        }
                        user_prompt = build_parse_repair_prompt(
                            stage_name,
                            schema_hint,
                            &payload,
                            &last_error,
                        );
                    }
                },
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

        bail!(
            "{stage_name} failed after {} attempts: {}",
            MAX_ATTEMPTS,
            last_error
        )
    }

    async fn generate_round_one(&self, request: &GenerationRequest) -> Result<GenerationResult> {
        let system_prompt = system_prompt_from_request(request);
        let mut usage = GenerationUsage::default();

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

        Ok(GenerationResult {
            response: GenerationResponse::Full {
                bodies: bodies_stage.bodies,
                foundry_config: config_stage.foundry_config,
            },
            usage,
        })
    }

    async fn generate_round_n(&self, request: &GenerationRequest) -> Result<GenerationResult> {
        let system_prompt = system_prompt_from_request(request);
        let mut usage = GenerationUsage::default();
        let user_prompt = user_prompt_from_request(request);

        let schema = build_body_schema(
            request.existing_bodies.as_ref(),
            request.existing_foundry_config.as_ref(),
        )?;

        let analysis: PatchAnalysisStage = self
            .request_json(
                &system_prompt,
                build_round_n_analysis_prompt(&schema, &Some(user_prompt))?,
                "patch analysis",
                "rootCause, configAdjustments, bodiesNeeded",
                &mut usage,
            )
            .await?;

        let existing_bodies = request
            .existing_bodies
            .as_ref()
            .context("round N requires existing bodies for patch generation")?;
        let relevant_bodies =
            extract_relevant_bodies(Some(existing_bodies), &analysis.bodies_needed)?
                .context("round N analysis requested bodies but none could be derived")?;

        let existing_config = request
            .existing_foundry_config
            .as_ref()
            .context("round N requires existing foundry config for patch generation")?;

        let patch_prompt =
            build_round_n_patch_prompt(&analysis, &relevant_bodies, existing_config)?;

        let response = self
            .request_generation_response(
                &system_prompt,
                patch_prompt,
                "round n patch",
                "mode=patch with bodies_updates + foundry_config_updates",
                &mut usage,
            )
            .await?;

        Ok(GenerationResult { response, usage })
    }
}

fn validate_generation_response(response: &GenerationResponse) -> Result<()> {
    match response {
        GenerationResponse::Full {
            bodies,
            foundry_config: _,
        } => validate_full_generation(bodies),
        GenerationResponse::Patch {
            bodies_updates,
            foundry_config_updates,
        } => validate_patch_updates(bodies_updates, foundry_config_updates),
    }
}

fn validate_full_generation(bodies: &BodiesJson) -> Result<()> {
    let mut issues = Vec::new();

    let has_basehandler_import = bodies
        .handler
        .imports
        .iter()
        .any(|line| line.contains("BaseHandler") && line.contains("src/base/BaseHandler.sol"));
    if !has_basehandler_import {
        issues.push(
            "handler imports must include BaseHandler from src/base/BaseHandler.sol".to_string(),
        );
    }
    for line in &bodies.handler.imports {
        if line.contains("foundry-huff/BaseHandler.sol") {
            issues.push("do not import BaseHandler from foundry-huff".to_string());
            break;
        }
    }

    if !bodies.handler.target_selectors.trim().is_empty()
        && !bodies
            .handler
            .target_selectors
            .trim_start()
            .starts_with("function")
    {
        issues.push("handler.targetSelectors must be empty or a function definition".to_string());
    }

    for name in bodies.invariant_test.invariants.keys() {
        if !name.starts_with("invariant_") {
            issues.push(format!("invariant name must start with invariant_: {name}"));
        }
    }

    if issues.is_empty() {
        return Ok(());
    }

    bail!(issues.join("; "))
}

fn validate_patch_updates(
    bodies_updates: &[crate::shared::models::JsonBlockUpdate],
    foundry_config_updates: &[crate::shared::models::JsonBlockUpdate],
) -> Result<()> {
    let mut invalid = Vec::new();
    for update in bodies_updates {
        if update.path.starts_with("bodies.")
            || update.path.starts_with("foundry_config.")
            || update.path.starts_with("Foundry.")
        {
            invalid.push(update.path.clone());
        }
        if let Some(name) = update.path.strip_prefix("invariantTest.invariants.") {
            if !name.starts_with("invariant_") {
                invalid.push(update.path.clone());
            }
        }
        if update.path == "handler.targetSelectors" {
            if let Some(value) = update.value.as_str() {
                if !value.trim().is_empty() && !value.trim_start().starts_with("function") {
                    invalid.push(update.path.clone());
                }
            }
        }
    }
    for update in foundry_config_updates {
        if update.path.starts_with("bodies.")
            || update.path.starts_with("foundry_config.")
            || !update.path.starts_with("Foundry.")
        {
            invalid.push(update.path.clone());
        }
    }

    if invalid.is_empty() {
        return Ok(());
    }

    bail!(
        "invalid patch paths or values (check Foundry prefix, invariant_ names, targetSelectors function): {}",
        invalid.join(", ")
    )
}

fn extract_relevant_bodies(
    existing: Option<&BodiesJson>,
    needed: &[String],
) -> Result<Option<BodiesJson>> {
    let Some(existing) = existing else {
        return Ok(None);
    };

    let mut filtered = existing.clone();
    let needed_set: std::collections::HashSet<String> = needed.iter().cloned().collect();

    filtered
        .handler
        .functions
        .retain(|name, _| needed_set.contains(name));
    filtered
        .invariant_test
        .invariants
        .retain(|name, _| needed_set.contains(name));

    if !needed.is_empty() {
        let missing: Vec<String> = needed
            .iter()
            .filter(|name| {
                !existing.handler.functions.contains_key(*name)
                    && !existing.invariant_test.invariants.contains_key(*name)
            })
            .cloned()
            .collect();

        if !missing.is_empty() {
            bail!("requested bodies not found: {}", missing.join(", "));
        }
    }

    Ok(Some(filtered))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use indexmap::IndexMap;

    use crate::generator::ports::outbound::LlmClientPort;
    use crate::shared::models::{
        AssembledPrompt, BodiesJson, BodiesMeta, FoundryConfig, HandlerBodies, InvariantTestBodies,
        Message, Role,
    };

    use super::{extract_relevant_bodies, LiteLlmGenerationAdapter};
    use crate::generator::ports::outbound::{GenerationPort, GenerationRequest};

    fn sample_bodies() -> BodiesJson {
        let mut functions = IndexMap::new();
        functions.insert("deposit".to_string(), "// deposit".to_string());
        functions.insert("withdraw".to_string(), "// withdraw".to_string());

        let mut invariants = IndexMap::new();
        invariants.insert("invariant_balance".to_string(), "assert(true);".to_string());

        BodiesJson {
            meta: BodiesMeta {
                contract: "Vault".to_string(),
                contract_path: "src/Vault.sol".to_string(),
                solidity: "^0.8.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            handler: HandlerBodies {
                contract_name: "VaultHandler".to_string(),
                output_path: "test/handlers/VaultHandler.sol".to_string(),
                imports: vec![],
                state_vars: vec![],
                ghost_vars: vec!["uint256 ghost_totalDeposited;".to_string()],
                constructor_signature: "constructor(address _vault)".to_string(),
                constructor_body: vec![],
                functions,
                target_selectors: "selectors".to_string(),
            },
            invariant_test: InvariantTestBodies {
                contract_name: "VaultInvariantTest".to_string(),
                output_path: "test/invariants/VaultInvariantTest.sol".to_string(),
                imports: vec![],
                state_vars: vec![],
                set_up_body: vec![],
                invariants,
            },
        }
    }

    fn sample_config() -> FoundryConfig {
        let mut weights = HashMap::new();
        weights.insert("deposit".to_string(), 0.5);
        weights.insert("withdraw".to_string(), 0.5);
        FoundryConfig {
            depth: 10,
            runs: 100,
            seed: "0xdeadbeef".to_string(),
            max_test_rejects: 10,
            dictionary_weight: 40,
            call_sequence_weights: weights,
            current_toml: None,
        }
    }

    #[test]
    fn filters_relevant_bodies() {
        let bodies = sample_bodies();
        let filtered = extract_relevant_bodies(Some(&bodies), &["deposit".to_string()])
            .expect("filter")
            .expect("some");

        assert!(filtered.handler.functions.contains_key("deposit"));
        assert!(!filtered.handler.functions.contains_key("withdraw"));
        assert!(filtered.invariant_test.invariants.is_empty());
    }

    struct MockClient {
        responses: Arc<Mutex<VecDeque<String>>>,
    }

    impl MockClient {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(VecDeque::from(responses))),
            }
        }
    }

    #[async_trait]
    impl LlmClientPort for MockClient {
        async fn complete(
            &self,
            _system_prompt: &str,
            _user_prompt: &str,
        ) -> anyhow::Result<(
            String,
            Option<crate::generator::domain::generation_response::GenerationUsage>,
        )> {
            let mut guard = self.responses.lock().expect("lock responses");
            let response = guard.pop_front().expect("expected mock response");
            Ok((response, None))
        }
    }

    #[tokio::test]
    async fn generates_round_n_patch_via_stages() {
        let analysis_payload = r#"{
            "rootCause": "ghost order",
            "configAdjustments": [],
            "bodiesNeeded": ["deposit"]
        }"#;

        let patch_payload = r#"{
            "mode": "patch",
            "bodies_updates": [
                {"op": "modify", "path": "handler.functions.deposit", "value": "function deposit(){}", "reason": "fix"}
            ],
            "foundry_config_updates": []
        }"#;

        let client = Box::new(MockClient::new(vec![
            analysis_payload.to_string(),
            patch_payload.to_string(),
        ]));

        let adapter = LiteLlmGenerationAdapter::new("openai/mock", "test", client);

        let assembled = AssembledPrompt {
            messages: vec![
                Message {
                    role: Role::System,
                    content: "system".to_string(),
                },
                Message {
                    role: Role::User,
                    content: "Round: 2\n\nFUZZ OUTPUT: test".to_string(),
                },
            ],
            round: 2,
            context_sections: vec![],
        };

        let request = GenerationRequest {
            round: 2,
            source_code: "contract Vault{}".to_string(),
            prompt: assembled,
            existing_bodies: Some(sample_bodies()),
            existing_foundry_config: Some(sample_config()),
        };

        let result = adapter.generate(request).await.expect("generate");
        match result.response {
            crate::generator::domain::generation_response::GenerationResponse::Patch {
                bodies_updates,
                foundry_config_updates,
            } => {
                assert_eq!(bodies_updates.len(), 1);
                assert!(foundry_config_updates.is_empty());
            }
            _ => panic!("expected patch response"),
        }
    }
}

#[async_trait]
impl GenerationPort for LiteLlmGenerationAdapter {
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResult> {
        self.set_api_key();

        if request.round == 1 {
            self.generate_round_one(&request).await
        } else {
            self.generate_round_n(&request).await
        }
    }
}
