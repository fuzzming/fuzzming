use anyhow::{Context, Result};

use crate::llm::ports::LlmGenerationRequest;
use crate::shared::models::{BodiesJson, Role};

use super::stages::AnalysisStage;

pub fn system_prompt_from_request(request: &LlmGenerationRequest) -> String {
    request
        .prompt
        .messages
        .iter()
        .find(|m| matches!(m.role, Role::System))
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

pub fn build_round_n_prompt(request: &LlmGenerationRequest) -> Result<String> {
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
