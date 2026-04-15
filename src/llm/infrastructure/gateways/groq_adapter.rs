use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use litellm_rs::{completion, system_message, user_message, CompletionOptions};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;

use crate::interfaces::artifacts::{BodiesJson, FoundryConfig, Role};
use crate::llm::application::ports::{
    LlmGenerationPort, LlmGenerationRequest, LlmGenerationResponse,
};

pub struct GroqAdapter {
    api_key: String,
    default_model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

const MAX_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalysisStage {
    // Using Value to prevent parsing errors when LLM returns objects instead of strings
    vulnerability_analysis: Vec<serde_json::Value>,
    handler_logic_pseudocode: serde_json::Value,
    invariant_mathematical_proofs: Vec<serde_json::Value>,
    critical_invariants: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BodiesStage {
    bodies: BodiesJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigStage {
    foundry_config: FoundryConfig,
}

impl GroqAdapter {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            default_model: "openai/gpt-oss-120b".to_string(),
            temperature: Some(0.1),
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

    fn build_system_prompt(&self, source_code: &str) -> String {
        format!(
            "You are a Senior Smart-Contract Security Researcher and Foundry Fuzzing Expert.\n\
             SOURCE_CODE:\n{}\n\n\
             STRICT OPERATIONAL RULES:\n\
             1. NO FOR-IN LOOPS: Solidity mappings are not iterable. You MUST use a ghost array 'address[] public actors' and push msg.sender to it.\n\
             2. PHYSICAL VS LOGICAL: Always compare internal state (totalAssets) against physical balances (asset.balanceOf(address(this))).\n\
             3. NAMESPACING: Handler contract must be named 'Handler' or '[Target]Handler'.\n\
             4. USE INDEXMAP ORDER: Generate JSON keys in the order they should appear in Solidity.\n\
             5. OUTPUT: Return valid JSON only.",
            source_code
        )
    }

    fn build_round_one_analysis_prompt(&self, request: &LlmGenerationRequest) -> String {
        format!(
            "Stage 1/3: Security Analysis & Logic Design.\n\
             Analyze: Ghost borrowing, Inflation attacks, and rounding errors.\n\
             \n\
             Return this JSON exactly:\n\
             {{\n\
               \"vulnerability_analysis\": [\"string\"],\n\
               \"handler_logic_pseudocode\": \"string describing state tracking\",\n\
               \"invariant_mathematical_proofs\": [\"string\"],\n\
               \"critical_invariants\": [\"string\"]\n\
             }}"
        )
    }

    fn build_round_one_bodies_prompt(&self, analysis: &AnalysisStage) -> Result<String> {
        // Convert the flexible analysis Value back into a pretty string for the LLM to read
        let analysis_summary = serde_json::to_string_pretty(analysis)?;

        Ok(format!(
            "Stage 2/3: Solidity Generation.\n\
\n\
Based on your previous security analysis, generate the full implementation of the Handler and Invariant test suite. Your output MUST be a valid JSON object matching the internal Rust schema exactly.\n\
\n\
STRICT DESIGN RULES:\n\
1. EXTERNAL CALLS ONLY: Handler functions MUST make external calls to the target contract instance (e.g., `vault.deposit{{value: msg.value}}()`). Do NOT reimplement the target contract's internal logic inside the handler.\n\
2. NO HALLUCINATIONS: Do not call functions or read variables on the target contract that do not explicitly exist in the provided source code.\n\
3. NO REDUNDANCIES: Do not write meaningless logic or checks, like `require(myUint >= 0)` (since uint256 cannot be negative).\n\
\n\
STRICT SCHEMA RULES:\n\
\n\
Case Sensitivity: Use camelCase for all keys (e.g., contractName, setUpBody, invariantTest).\n\
\n\
Structural Integrity: Do not combine code into a single field. Break it down into the arrays and objects specified below.\n\
\n\
IndexMap Logic: The functions and invariants keys must be JSON Objects (key-value maps) where the value is the full function body as a string.\n\
\n\
No for-in loops: Use the actors array pattern in your logic.\n\
\n\
REQUIRED JSON STRUCTURE:\n\
{{\n\
  \"bodies\": {{\n\
    \"meta\": {{\n\
      \"contract\": \"TargetContractName\",\n\
      \"contractPath\": \"path/to/Target.sol\",\n\
      \"solidity\": \"solidity_version_string\",\n\
      \"generatedAt\": \"timestamp\"\n\
    }},\n\
    \"handler\": {{\n\
      \"contractName\": \"HandlerName\",\n\
      \"outputPath\": \"path/to/Handler.sol\",\n\
      \"imports\": [\"array\", \"of\", \"import\", \"lines\"],\n\
      \"stateVars\": [\"array\", \"of\", \"state\", \"variables\"],\n\
      \"ghostVars\": [\"array\", \"of\", \"ghost\", \"variables\"],\n\
      \"constructorSignature\": \"signature_string\",\n\
      \"constructorBody\": [\"array\", \"of\", \"solidity\", \"lines\"],\n\
      \"functions\": {{\n\
        \"functionName\": \"full_solidity_function_string\"\n\
      }},\n\
      \"targetSelectors\": \"selector_expression_string\"\n\
    }},\n\
    \"invariantTest\": {{\n\
      \"contractName\": \"TestName\",\n\
      \"outputPath\": \"path/to/Test.sol\",\n\
      \"imports\": [\"array\", \"of\", \"import\", \"lines\"],\n\
      \"stateVars\": [\"array\", \"of\", \"state\", \"variables\"],\n\
      \"setUpBody\": [\"array\", \"of\", \"setup\", \"lines\"],\n\
      \"invariants\": {{\n\
        \"invariantName\": \"full_solidity_function_string\"\n\
      }}\n\
    }}\n\
  }}\n\
}}\n\
\n\
Analysis Context:\n\
{}\n",
            analysis_summary
        ))
    }

    fn build_round_one_config_prompt(
        &self,
        analysis: &AnalysisStage,
        bodies: &BodiesJson,
    ) -> Result<String> {
        let analysis_json =
            serde_json::to_string_pretty(analysis).context("failed to serialize analysis stage")?;
        let function_names: Vec<&String> = bodies.handler.functions.keys().collect();
        let functions_json = serde_json::to_string_pretty(&function_names)
            .context("failed to serialize handler function names")?;

        Ok(format!(
            "Stage 3/3: generate Foundry config only.\n\
             Return this exact JSON shape:\n\
             {{\n\
               \"foundry_config\": {{\n\
                 \"depth\": integer,\n\
                 \"runs\": integer,\n\
                 \"seed\": \"0x...\",\n\
                 \"max_test_rejects\": integer,\n\
                 \"dictionary_weight\": integer,\n\
                 \"call_sequence_weights\": {{\"handlerFunctionName\": float}}\n\
               }}\n\
             }}\n\
             \n\
             Guidance:\n\
             - call_sequence_weights keys must match handler function names exactly.\n\
             - Weights should be realistic and sum near 1.0.\n\
             - Choose runs/depth for meaningful state exploration.\n\
             \n\
             Analysis JSON:\n{}\n\
             \n\
             Handler function names:\n{}",
            analysis_json, functions_json
        ))
    }

    fn build_round_n_prompt(&self, request: &LlmGenerationRequest) -> Result<String> {
        let existing_bodies_json = serde_json::to_string_pretty(&request.existing_bodies)
            .context("failed to serialize existing bodies")?;
        let existing_config_json = serde_json::to_string_pretty(&request.existing_foundry_config)
            .context("failed to serialize existing foundry config")?;

        Ok(format!(
            "Round: {}\n\
             Return JSON only.\n\
             \n\
             If round is 1, return:\n\
             {{\n\
               \"mode\":\"full\",\n\
               \"bodies\": {{...}},\n\
               \"foundry_config\": {{...}}\n\
             }}\n\
             \n\
             If round > 1, prefer patch mode:\n\
             {{\n\
               \"mode\":\"patch\",\n\
               \"bodies_updates\":[{{\"path\":\"string\",\"value\":any,\"reason\":\"string\"}}],\n\
               \"foundry_config_updates\":[{{\"path\":\"string\",\"value\":any,\"reason\":\"string\"}}]\n\
             }}\n\
             \n\
             Existing bodies:\n{}\n\
             \n\
             Existing foundry config:\n{}",
            request.round, existing_bodies_json, existing_config_json
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

    async fn complete_once(
        &self,
        model: &str,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String> {
        let response = completion(
            model,
            vec![
                system_message(system_prompt.to_string()),
                user_message(user_prompt.to_string()),
            ],
            Some(self.build_options()),
        )
        .await
        .map_err(|e| anyhow!("litellm completion failed: {e}"))?;

        response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .map(|content| content.to_string())
            .ok_or_else(|| anyhow!("groq adapter returned empty content"))
    }

    async fn request_json<T>(
        &self,
        model: &str,
        system_prompt: &str,
        initial_prompt: String,
        stage_name: &str,
        schema_hint: &str,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let mut user_prompt = initial_prompt;
        let mut last_error = String::new();

        for attempt in 1..=MAX_ATTEMPTS {
            let content = self
                .complete_once(model, system_prompt, &user_prompt)
                .await?;
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

    async fn generate_round_one_chained(
        &self,
        request: &LlmGenerationRequest,
    ) -> Result<LlmGenerationResponse> {
        let model = self.pick_model(&request.model);
        let system_prompt = self.build_system_prompt(&request.source_code);

        // STEP 1: Deep Analysis (The "Thinking" Phase)
        let analysis: AnalysisStage = self
            .request_json(
                &model,
                &system_prompt,
                self.build_round_one_analysis_prompt(request),
                "analysis",
                "vulnerability_analysis, handler_logic_pseudocode, invariant_mathematical_proofs",
            )
            .await?;

        // Prevent burst rate limits (Groq strict free tier)
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // STEP 2: Body Generation (The "Coding" Phase - Informed by Step 1)
        let bodies_stage: BodiesStage = self
            .request_json(
                &model,
                &system_prompt,
                self.build_round_one_bodies_prompt(&analysis)?,
                "bodies",
                "bodies object with valid Solidity syntax",
            )
            .await?;

        // Prevent burst rate limits
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // STEP 3: Config Generation
        let config_stage: ConfigStage = self
            .request_json(
                &model,
                &system_prompt,
                self.build_round_one_config_prompt(&analysis, &bodies_stage.bodies)?,
                "config",
                "foundry_config mapping to handler functions",
            )
            .await?;

        Ok(LlmGenerationResponse::Full {
            bodies: bodies_stage.bodies,
            foundry_config: config_stage.foundry_config,
        })
    }
}

#[async_trait]
impl LlmGenerationPort for GroqAdapter {
    async fn generate(&self, request: LlmGenerationRequest) -> Result<LlmGenerationResponse> {
        std::env::set_var("GROQ_API_KEY", &self.api_key);

        if request.round == 1 {
            return self.generate_round_one_chained(&request).await;
        }

        let system_prompt = self.build_system_prompt(&request.source_code);
        let mut user_prompt = self.build_round_n_prompt(&request)?;
        let model = self.pick_model(&request.model);

        let mut last_error = String::new();

        for attempt in 1..=MAX_ATTEMPTS {
            let content = self
                .complete_once(&model, &system_prompt, &user_prompt)
                .await?;
            let payload = extract_json_payload(&content)?;

            match parse_generation_response(&payload) {
                Ok(parsed) => return Ok(parsed),
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

fn build_parse_repair_prompt(
    stage_name: &str,
    schema_hint: &str,
    previous_payload: &str,
    parse_error: &str,
) -> String {
    format!(
        "Your {stage_name} JSON is invalid. Repair it.\n\
         Return JSON only, no markdown.\n\
         Required shape: {schema_hint}\n\
         Parse error: {parse_error}\n\
         Invalid payload:\n{previous_payload}"
    )
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
                        if let Some(foundry_config_updates) =
                            patch_obj.get("foundry_config_updates")
                        {
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
        let raw =
            "```json\n{\"mode\":\"patch\",\"bodies_updates\":[],\"foundry_config_updates\":[]}\n```";
        let out = extract_json_payload(raw).expect("must parse fence");
        assert_eq!(
            out,
            "{\"mode\":\"patch\",\"bodies_updates\":[],\"foundry_config_updates\":[]}"
        );
    }

    #[test]
    fn normalizes_nested_patch_envelope() {
        let payload = r#"{
            "mode": "patch",
            "patch": {
                "bodies_updates": [],
                "foundry_config_updates": []
            }
        }"#;

        let parsed = parse_generation_response(payload).expect("must parse normalized patch");
        assert!(matches!(parsed, LlmGenerationResponse::Patch { .. }));
    }
}
