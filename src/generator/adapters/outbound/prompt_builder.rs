use anyhow::{Context, Result};

use crate::generator::ports::outbound::GenerationRequest;
use crate::shared::models::{BodiesJson, FoundryConfig, Role};

use super::stages::{AnalysisStage, PatchAnalysisStage};

pub fn system_prompt_from_request(request: &GenerationRequest) -> String {
    request
        .prompt
        .messages
        .iter()
        .find(|m| matches!(m.role, Role::System))
        .map(|m| m.content.clone())
        .unwrap_or_default()
}

pub fn user_prompt_from_request(request: &GenerationRequest) -> String {
    request
    .prompt
    .messages
    .iter()
    .find(|m| matches!(m.role, Role::User))
    .map(|m| m.content.clone())
    .unwrap_or_default()
}

pub fn build_round_one_analysis_prompt() -> String {
    "Stage 1/3: Security Analysis & Logic Design.\n\
     Analyze: Ghost borrowing, Inflation attacks, and rounding errors.\n\
     \n\
     Return this JSON exactly:\n\
     {\n\
       \"vulnerability_analysis\": [\"string\"],\n\
       \"handler_logic_pseudocode\": \"string describing state tracking\",\n\
       \"invariant_mathematical_proofs\": [\"string\"],\n\
       \"critical_invariants\": [\"string\"]\n\
     }"
    .to_string()
}

pub fn build_round_one_bodies_prompt(analysis: &AnalysisStage) -> Result<String> {
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

pub fn build_round_one_config_prompt(
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

pub fn build_round_n_prompt(request: &GenerationRequest) -> Result<String> {
    let existing_bodies_json = serde_json::to_string_pretty(&request.existing_bodies)
        .context("failed to serialize existing bodies")?;
    let existing_config_json = serde_json::to_string_pretty(&request.existing_foundry_config)
        .context("failed to serialize existing foundry config")?;

    Ok(format!(
        "Round: {}\n\
         Return JSON only. No markdown, no prose, no code fences.\n\
         \n\
         STRICT OUTPUT CONTRACT:\n\
         - If round == 1, return exactly:\n\
                     {{\n\
           \"mode\":\"full\",\n\
           \"bodies\": {{...}},\n\
           \"foundry_config\": {{...}}\n\
                     }}\n\
         - If round > 1, you MUST return exactly:\n\
                     {{\n\
           \"mode\":\"patch\",\n\
               \"bodies_updates\":[{{\"op\":\"add|modify|remove\",\"path\":\"string\",\"value\":any,\"reason\":\"string\"}}],\n\
               \"foundry_config_updates\":[{{\"op\":\"add|modify|remove\",\"path\":\"string\",\"value\":any,\"reason\":\"string\"}}]\n\
                     }}\n\
         \n\
         PATCH RULES (round > 1):\n\
         1. Multiple patches are allowed: each updates array may contain 0..N items.\n\
         2. Each patch item MUST contain exactly 4 keys: op, path, value, reason.\n\
         3. path MUST be a dot-path to the field being replaced.\n\
         4. op MUST be one of add, modify, remove.\n\
         5. add requires target key missing; modify requires existing key replacement; remove deletes existing key.\n\
         6. For remove, set value to null.\n\
         7. Do not include duplicate path entries in the same response.\n\
         8. If no change is required for one artifact, return that artifact updates as [].\n\
         9. Never return nested wrappers like {{\"patch\":{{...}}}} or {{\"full\":{{...}}}}.\n\
         \n\
         VALID bodies path prefixes:\n\
         - meta.contract\n\
         - meta.contractPath\n\
         - meta.solidity\n\
         - meta.generatedAt\n\
         - handler.contractName\n\
         - handler.outputPath\n\
         - handler.imports\n\
         - handler.stateVars\n\
         - handler.ghostVars\n\
         - handler.constructorSignature\n\
         - handler.constructorBody\n\
         - handler.functions.<functionName>\n\
         - handler.targetSelectors\n\
         - invariantTest.contractName\n\
         - invariantTest.outputPath\n\
         - invariantTest.imports\n\
         - invariantTest.stateVars\n\
         - invariantTest.setUpBody\n\
         - invariantTest.invariants.<invariantName>\n\
         \n\
         VALID foundry_config path prefixes:\n\
         - depth\n\
         - runs\n\
         - seed\n\
         - max_test_rejects\n\
         - dictionary_weight\n\
         - call_sequence_weights.<handlerFunctionName>\n\
         - current_toml\n\
         \n\
         Examples:\n\
         - bodies update path: \"handler.functions.deposit\"\n\
         - config update path: \"call_sequence_weights.withdraw\"\n\
         \n\
         Existing bodies:\n{}\n\
         \n\
         Existing foundry config:\n{}",
        request.round, existing_bodies_json, existing_config_json
    ))
}

pub fn build_body_schema(
    existing_bodies: Option<&BodiesJson>,
    existing_config: Option<&FoundryConfig>,
) -> Result<String> {
    let mut schema = serde_json::Map::new();

    if let Some(bodies) = existing_bodies {
        let handler_functions: Vec<String> = bodies.handler.functions.keys().cloned().collect();
        let invariants: Vec<String> = bodies.invariant_test.invariants.keys().cloned().collect();

        schema.insert(
            "handler".to_string(),
            serde_json::json!({
                "contractName": bodies.handler.contract_name,
                "functions": handler_functions,
                "ghostVars": bodies.handler.ghost_vars,
            }),
        );
        schema.insert(
            "invariantTest".to_string(),
            serde_json::json!({
                "contractName": bodies.invariant_test.contract_name,
                "invariants": invariants,
            }),
        );
    }

    if let Some(config) = existing_config {
        schema.insert(
            "foundry_config".to_string(),
            serde_json::json!({
                "depth": config.depth,
                "runs": config.runs,
                "call_sequence_weights": config.call_sequence_weights,
            }),
        );
    }

    Ok(serde_json::to_string_pretty(&schema)?)
}

pub fn build_round_n_analysis_prompt(schema: &str, fuzz_feedback: &Option<String>) -> Result<String> {
    let feedback = fuzz_feedback
        .as_deref()
        .unwrap_or("No fuzz feedback provided.");

    Ok(format!(
        "Stage 1/2: Patch Analysis.\n\
Return JSON only. No markdown or prose.\n\
\n\
Analyze the fuzz feedback and compact schema below.\n\
Identify root cause, affected paths, config adjustments, and which bodies must be included.\n\
\n\
REQUIRED JSON SHAPE (camelCase):\n\
{{\n\
  \"rootCause\": \"string\",\n\
  \"affectedPaths\": [\"dot.path\"],\n\
  \"configAdjustments\": [{{\"path\":\"dot.path\",\"reason\":\"string\"}}],\n\
  \"bodiesNeeded\": [\"functionOrInvariantName\"],\n\
  \"noChangeNeeded\": [\"functionOrInvariantName\"]\n\
}}\n\
\n\
Compact schema:\n{}\n\
\n\
Fuzz feedback:\n{}",
        schema, feedback
    ))
}

pub fn build_round_n_patch_prompt(
    analysis: &PatchAnalysisStage,
    relevant_bodies: &BodiesJson,
    existing_config: &FoundryConfig,
) -> Result<String> {
    let analysis_json = serde_json::to_string_pretty(analysis)?;
    let bodies_json = serde_json::to_string_pretty(relevant_bodies)?;
    let config_json = serde_json::to_string_pretty(existing_config)?;

    Ok(format!(
        "Stage 2/2: Targeted Patch.\n\
Return JSON only. No markdown, no prose, no code fences.\n\
\n\
STRICT OUTPUT CONTRACT (round > 1):\n\
{{\n\
  \"mode\":\"patch\",\n\
  \"bodies_updates\":[{{\"op\":\"add|modify|remove\",\"path\":\"string\",\"value\":any,\"reason\":\"string\"}}],\n\
  \"foundry_config_updates\":[{{\"op\":\"add|modify|remove\",\"path\":\"string\",\"value\":any,\"reason\":\"string\"}}]\n\
}}\n\
\n\
PATCH RULES:\n\
1. Multiple patches are allowed: each updates array may contain 0..N items.\n\
2. Each patch item MUST contain exactly 4 keys: op, path, value, reason.\n\
3. path MUST be a dot-path to the field being replaced.\n\
4. op MUST be one of add, modify, remove.\n\
5. add requires target key missing; modify requires existing key replacement; remove deletes existing key.\n\
6. For remove, set value to null.\n\
7. Do not include duplicate path entries in the same response.\n\
8. If no change is required for one artifact, return that artifact updates as [].\n\
9. Never return nested wrappers like {{\"patch\":{{...}}}} or {{\"full\":{{...}}}}.\n\
\n\
VALID bodies path prefixes:\n\
- meta.contract\n\
- meta.contractPath\n\
- meta.solidity\n\
- meta.generatedAt\n\
- handler.contractName\n\
- handler.outputPath\n\
- handler.imports\n\
- handler.stateVars\n\
- handler.ghostVars\n\
- handler.constructorSignature\n\
- handler.constructorBody\n\
- handler.functions.<functionName>\n\
- handler.targetSelectors\n\
- invariantTest.contractName\n\
- invariantTest.outputPath\n\
- invariantTest.imports\n\
- invariantTest.stateVars\n\
- invariantTest.setUpBody\n\
- invariantTest.invariants.<invariantName>\n\
\n\
VALID foundry_config path prefixes:\n\
- depth\n\
- runs\n\
- seed\n\
- max_test_rejects\n\
- dictionary_weight\n\
- call_sequence_weights.<handlerFunctionName>\n\
- current_toml\n\
\n\
Analysis JSON:\n{}\n\
\n\
Relevant bodies:\n{}\n\
\n\
Existing foundry config:\n{}",
        analysis_json, bodies_json, config_json
    ))
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use std::collections::HashMap;

    use crate::shared::models::{
        BodiesJson, BodiesMeta, FoundryConfig, HandlerBodies, InvariantTestBodies,
    };

    use super::build_body_schema;

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
    fn builds_compact_schema() {
        let schema = build_body_schema(Some(&sample_bodies()), Some(&sample_config()))
            .expect("schema built");
        let value: serde_json::Value = serde_json::from_str(&schema).expect("schema json");

        assert_eq!(value["handler"]["contractName"], "VaultHandler");
        assert!(value["handler"]["functions"].as_array().unwrap().len() >= 2);
        assert_eq!(value["invariantTest"]["contractName"], "VaultInvariantTest");
        assert!(value["foundry_config"]["call_sequence_weights"].is_object());
    }
}
