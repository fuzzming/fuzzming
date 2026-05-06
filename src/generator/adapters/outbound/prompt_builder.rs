use anyhow::{Context, Result};

use crate::generator::ports::outbound::GenerationRequest;
use crate::shared::models::{BodiesJson, Role};

use super::stages::AnalysisStage;

pub fn system_prompt_from_request(request: &GenerationRequest) -> String {
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

pub fn build_round_one_bodies_prompt(
    analysis: &AnalysisStage,
    contract_name: &str,
    contract_path: &str,
) -> Result<String> {
    let analysis_summary = serde_json::to_string_pretty(analysis)?;
    let handler_name = format!("{}Handler", contract_name);
    let test_name = format!("{}InvariantTest", contract_name);

    // Import lines the LLM must use — derived by FuzzMing, not chosen by the LLM.
    let handler_target_import = format!(
        "import {{{{{}}}}} from \"{}\";",
        contract_name, contract_path
    );
    let test_handler_import = format!(
        "import {{{{{}}}}} from \"./{}.sol\";",
        handler_name, handler_name
    );
    let test_std_import = "import {Test} from \"forge-std/Test.sol\";";

    Ok(format!(
        "Stage 2/3: Solidity Generation.\n\
\n\
Based on your previous security analysis, generate the full implementation of the Handler and \
Invariant test suite. Your output MUST be a valid JSON object matching the schema exactly.\n\
\n\
FILE LAYOUT (fixed — do not invent other paths):\n\
  Handler:        test/fuzzming/{contract_name}/{handler_name}.sol\n\
  Invariant test: test/fuzzming/{contract_name}/{test_name}.sol\n\
  Both files are in the same directory. Use relative imports between them.\n\
\n\
CONTRACT NAMES (use exactly these — do not vary capitalisation or suffixes):\n\
  handler.contractName:      \"{handler_name}\"\n\
  invariantTest.contractName: \"{test_name}\"\n\
  meta.contract:              \"{contract_name}\"\n\
  meta.contractPath:          \"{contract_path}\"\n\
\n\
REQUIRED IMPORT LINES:\n\
  In {handler_name}.imports, you MUST include:\n\
    \"{handler_target_import}\"\n\
    \"{test_std_import}\"\n\
  Do NOT import Token, ERC20, or any other dependency — the target contract manages its own imports.\n\
  In {test_name}.imports, you MUST include:\n\
    \"{test_handler_import}\"\n\
    \"{test_std_import}\"\n\
    \"{handler_target_import}\"\n\
\n\
CONTRACT INHERITANCE (mandatory):\n\
  {handler_name} must inherit from Test: write `contract {handler_name} is Test {{`\n\
  {test_name} must inherit from Test: write `contract {test_name} is Test {{`\n\
\n\
STRICT DESIGN RULES:\n\
1. EXTERNAL CALLS ONLY: Handler functions MUST make external calls to the target contract \
instance. Do NOT reimplement the target contract's internal logic inside the handler.\n\
2. NO HALLUCINATIONS: Do not call functions or read variables on the target contract that do \
not explicitly exist in the provided source code.\n\
3. NO REDUNDANCIES: Do not write meaningless checks like `require(myUint >= 0)`.\n\
4. NO EXTRA IMPORTS: Only import what is listed in REQUIRED IMPORT LINES above. Never add imports for Token, IERC20, or any dependency of the target.
5. targetSelectors: Always set to empty string \"\". Target selector setup (targetSelector, targetContract) belongs ONLY in the invariant test's setUpBody — never in the handler.\n\
6. NO REDEFINING TEST HELPERS: Do not define functions already provided by inheriting Test — never write your own `bound`, `vm`, `makeAddr`, `deal`, or similar.\n\
7. NO RAW BYTECODE: Never embed hex bytecode in setUp. To deploy a dependency, use `deployCode(\"ContractName.sol:ContractName\")` — nothing else.\n\
\n\
STRICT SCHEMA RULES:\n\
- Use camelCase for all keys.\n\
- Do not combine code into a single field — use the arrays and objects specified below.\n\
- functions and invariants are JSON objects where the value is the full function body as a string.\n\
- Do not include outputPath — paths are managed by the tool, not the LLM.\n\
\n\
REQUIRED JSON STRUCTURE:\n\
{{\n\
    \"bodies\": {{\n\
        \"meta\": {{\n\
            \"contract\": \"{contract_name}\",\n\
            \"contractPath\": \"{contract_path}\",\n\
            \"solidity\": \"solidity_version_string\",\n\
            \"generatedAt\": \"timestamp\"\n\
        }},\n\
        \"handler\": {{\n\
            \"contractName\": \"{handler_name}\",\n\
            \"imports\": [\"array of import lines\"],\n\
            \"stateVars\": [\"array of state variables\"],\n\
            \"ghostVars\": [\"array of ghost variables\"],\n\
            \"constructorSignature\": \"signature_string\",\n\
            \"constructorBody\": [\"array of solidity lines\"],\n\
            \"functions\": {{\n\
                \"functionName\": \"full_solidity_function_string\"\n\
            }},\n\
            \"targetSelectors\": \"\"\n\
        }},\n\
        \"invariantTest\": {{\n\
            \"contractName\": \"{test_name}\",\n\
            \"imports\": [\"array of import lines\"],\n\
            \"stateVars\": [\"array of state variables\"],\n\
            \"setUpBody\": [\"array of setup lines\"],\n\
            \"invariants\": {{\n\
                \"invariantName\": \"full_solidity_function_string\"\n\
            }}\n\
        }}\n\
    }}\n\
}}\n\
\n\
Analysis Context:\n\
{analysis_summary}\n",
        contract_name = contract_name,
        contract_path = contract_path,
        handler_name = handler_name,
        test_name = test_name,
        handler_target_import = handler_target_import,
        test_handler_import = test_handler_import,
        analysis_summary = analysis_summary,
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
        "Round: {round}\n\
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
         - handler.imports\n\
         - handler.stateVars\n\
         - handler.ghostVars\n\
         - handler.constructorSignature\n\
         - handler.constructorBody\n\
         - handler.functions.<functionName>\n\
         - handler.targetSelectors\n\
         - invariantTest.contractName\n\
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
         Existing bodies:\n{existing_bodies}\n\
         \n\
         Existing foundry config:\n{existing_config}",
        round = request.round,
        existing_bodies = existing_bodies_json,
        existing_config = existing_config_json,
    ))
}
