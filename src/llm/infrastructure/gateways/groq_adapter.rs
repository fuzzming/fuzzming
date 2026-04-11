use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use litellm_rs::{completion, system_message, user_message, CompletionOptions};
use serde_json::json;

use crate::llm::application::ports::{LlmGenerationPort, LlmGenerationRequest, LlmGenerationResponse};

pub struct GroqAdapter {
    api_key: String,
    default_model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

impl GroqAdapter {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            default_model: "groq/llama-3.3-70b-versatile".to_string(),
            temperature: Some(0.2),
            max_tokens: Some(4_096),
        }
    }

    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn with_temperature(mut self, temperature: Option<f32>) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: Option<u32>) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    fn pick_model(&self, requested_model: &str) -> String {
        let model = if requested_model.trim().is_empty() {
            self.default_model.clone()
        } else {
            requested_model.to_string()
        };

        if model.starts_with("groq/") {
            model
        } else {
            format!("groq/{model}")
        }
    }

    fn build_system_prompt(&self, request: &LlmGenerationRequest) -> String {
        format!(
            "You are an invariant-fuzzing generation engine.\n\
             The Solidity source code is immutable across rounds.\n\
             Use the source code below as canonical context.\n\n\
             SOURCE_CODE_START\n{}\nSOURCE_CODE_END\n\n\
             Output rules:\n\
             1. Return JSON only (no markdown, no prose).\n\
             2. Round 1 must return mode=\"full\" with complete bodies and foundry_config.\n\
             3. Later rounds should prefer mode=\"patch\" and only update minimal blocks.\n\
             4. Keep Solidity snippets valid and self-contained.\n\
             5. Preserve existing unchanged blocks when suggesting patches.",
            request.source_code
        )
    }

    fn build_user_prompt(&self, request: &LlmGenerationRequest) -> Result<String> {
        let prompt_json = serde_json::to_string_pretty(&request.prompt)
            .context("failed to serialize assembled prompt")?;
        let existing_bodies_json = serde_json::to_string_pretty(&request.existing_bodies)
            .context("failed to serialize existing bodies")?;
        let existing_config_json = serde_json::to_string_pretty(&request.existing_foundry_config)
            .context("failed to serialize existing foundry config")?;

        Ok(format!(
                        "Round: {}\n\
                         Return JSON ONLY.\n\
                         IMPORTANT: DO NOT wrap fields inside nested `full` or `patch` objects.\n\
                         IMPORTANT: Top-level must contain mode plus either full payload fields OR patch arrays.\n\n\
                         EXACT FULL-MODE TEMPLATE:\n\
                         {{\n\
                             \"mode\": \"full\",\n\
                             \"bodies\": {{\n\
                                 \"meta\": {{\n\
                                     \"contract\": \"Vault\",\n\
                                     \"contractPath\": \"src/Vault.sol\",\n\
                                     \"solidity\": \"^0.8.20\",\n\
                                     \"generatedAt\": \"2026-01-01T00:00:00Z\"\n\
                                 }},\n\
                                 \"handler\": {{\n\
                                     \"contractName\": \"VaultHandler\",\n\
                                     \"outputPath\": \"test/handlers/VaultHandler.sol\",\n\
                                     \"imports\": [\"import {{Test}} from \\\"forge-std/Test.sol\\\";\"],\n\
                                     \"stateVars\": [\"Vault internal vault;\"],\n\
                                     \"ghostVars\": [\"uint256 internal ghostTotalDeposits;\"],\n\
                                     \"constructorSignature\": \"constructor(Vault _vault)\",\n\
                                     \"constructorBody\": [\"vault = _vault;\"],\n\
                                     \"functions\": {{\n\
                                         \"deposit\": \"function deposit(uint256 amount) external {{ ... }}\"\n\
                                     }},\n\
                                     \"targetSelectors\": \"function targetSelectors() public view returns (bytes4[] memory sels) {{ ... }}\"\n\
                                 }},\n\
                                 \"invariantTest\": {{\n\
                                     \"contractName\": \"VaultInvariantTest\",\n\
                                     \"outputPath\": \"test/invariants/VaultInvariantTest.sol\",\n\
                                     \"imports\": [\"import {{Test}} from \\\"forge-std/Test.sol\\\";\"],\n\
                                     \"stateVars\": [\"Vault internal vault;\", \"VaultHandler internal handler;\"],\n\
                                     \"setUpBody\": [\"vault = new Vault();\", \"handler = new VaultHandler(vault);\"],\n\
                                     \"invariants\": {{\n\
                                         \"balance_never_negative\": \"function invariant_balance_never_negative() public {{ ... }}\"\n\
                                     }}\n\
                                 }}\n\
                             }},\n\
                             \"foundry_config\": {{\n\
                                 \"depth\": 128,\n\
                                 \"runs\": 256,\n\
                                 \"seed\": \"0x1234\",\n\
                                 \"max_test_rejects\": 65536,\n\
                                 \"dictionary_weight\": 40,\n\
                                 \"call_sequence_weights\": {{\n\
                                     \"deposit\": 0.6,\n\
                                     \"withdraw\": 0.4\n\
                                 }}\n\
                             }}\n\
                         }}\n\n\
                         EXACT PATCH-MODE TEMPLATE:\n\
                         {{\n\
                             \"mode\": \"patch\",\n\
                             \"bodies_updates\": [{{\"path\": \"handler.functions.deposit\", \"value\": \"function ...\", \"reason\": \"...\"}}],\n\
                             \"foundry_config_updates\": [{{\"path\": \"runs\", \"value\": 512, \"reason\": \"...\"}}]\n\
                         }}\n\n\
                         AssembledPrompt:\n{}\n\n\
                         Existing bodies JSON (null for round 1):\n{}\n\n\
                         Existing foundry config JSON (null for round 1):\n{}\n\n\
                         For round=1 you MUST return mode=full and fill all required fields.",
            request.round, prompt_json, existing_bodies_json, existing_config_json
        ))
    }

    fn build_options(&self) -> CompletionOptions {
        let mut options = CompletionOptions::default();
        options.temperature = self.temperature;
        options.max_tokens = self.max_tokens;
        options.extra_params = HashMap::from([(
            "response_format".to_string(),
            json!({ "type": "json_object" }),
        )]);
        options
    }
}

#[async_trait]
impl LlmGenerationPort for GroqAdapter {
    async fn generate(&self, request: LlmGenerationRequest) -> Result<LlmGenerationResponse> {
        std::env::set_var("GROQ_API_KEY", &self.api_key);

        let system_prompt = self.build_system_prompt(&request);
        let user_prompt = self.build_user_prompt(&request)?;

        let model = self.pick_model(&request.model);

        let response = completion(
            &model,
            vec![system_message(system_prompt), user_message(user_prompt)],
            Some(self.build_options()),
        )
        .await
        .map_err(|e| anyhow!("litellm completion failed: {e}"))?;

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .map(|content| content.to_string())
            .ok_or_else(|| anyhow!("groq adapter returned empty content"))?;

        let payload = extract_json_payload(&content)?;
        let parsed = parse_generation_response(&payload)?;

        if request.round == 1 && !matches!(parsed, LlmGenerationResponse::Full { .. }) {
            bail!("round 1 must return mode=full");
        }

        Ok(parsed)
    }
}

fn extract_json_payload(raw: &str) -> Result<String> {
    let trimmed = raw.trim();

    if trimmed.starts_with("```") {
        let stripped = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        return Ok(stripped.to_string());
    }

    Ok(trimmed.to_string())
}

fn parse_generation_response(payload: &str) -> Result<LlmGenerationResponse> {
    let mut value: serde_json::Value = serde_json::from_str(payload)
        .with_context(|| format!("failed to parse structured response: {payload}"))?;

    normalize_envelope(&mut value);

    serde_json::from_value(value)
        .with_context(|| format!("failed to parse structured response: {payload}"))
}

fn normalize_envelope(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    let mode = obj
        .get("mode")
        .and_then(|m| m.as_str())
        .map(|m| m.to_string());

    match mode.as_deref() {
        Some("full") => {
            if let Some(full) = obj.remove("full") {
                if let Some(full_obj) = full.as_object() {
                    if !obj.contains_key("bodies") {
                        if let Some(bodies) = full_obj.get("bodies") {
                            obj.insert("bodies".to_string(), bodies.clone());
                        }
                    }
                    if !obj.contains_key("foundry_config") {
                        if let Some(foundry_config) = full_obj.get("foundry_config") {
                            obj.insert("foundry_config".to_string(), foundry_config.clone());
                        }
                    }
                }
            }
        }
        Some("patch") => {
            if let Some(patch) = obj.remove("patch") {
                if let Some(patch_obj) = patch.as_object() {
                    if !obj.contains_key("bodies_updates") {
                        if let Some(bodies_updates) = patch_obj.get("bodies_updates") {
                            obj.insert("bodies_updates".to_string(), bodies_updates.clone());
                        }
                    }
                    if !obj.contains_key("foundry_config_updates") {
                        if let Some(foundry_config_updates) = patch_obj.get("foundry_config_updates") {
                            obj.insert(
                                "foundry_config_updates".to_string(),
                                foundry_config_updates.clone(),
                            );
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markdown_code_fence() {
        let raw = "```json\n{\"mode\":\"patch\",\"bodies_updates\":[],\"foundry_config_updates\":[]}\n```";
        let out = extract_json_payload(raw).expect("must parse fence");
        assert_eq!(
            out,
            "{\"mode\":\"patch\",\"bodies_updates\":[],\"foundry_config_updates\":[]}"
        );
    }
}
