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
     Analyze the contract for ALL invariant-breaking vulnerability classes:\n\
     - State corruption: unauthorized state transitions, missing access control\n\
     - Arithmetic: overflow, underflow, rounding and truncation errors\n\
     - Asset accounting: balance tracking, share/asset ratio drift, ghost state divergence\n\
     - Access control: privileged functions reachable by unauthorized callers\n\
     - Business logic: properties that must hold across all valid state transitions\n\
     \n\
     Return this JSON exactly:\n\
     {\n\
       \"vulnerability_analysis\": [\"string — one entry per finding\"],\n\
       \"handler_logic_pseudocode\": \"string describing what state the handler must track\",\n\
       \"invariant_mathematical_proofs\": [\"string — one entry per invariant\"],\n\
       \"critical_invariants\": [\"string\"]\n\
     }"
    .to_string()
}

fn extract_pragma(source: &str) -> String {
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("pragma solidity") {
            return t
                .trim_end_matches(';')
                .trim_start_matches("pragma solidity")
                .trim()
                .to_string();
        }
    }
    "^0.8.20".to_string()
}

pub fn build_round_one_bodies_prompt(
    analysis: &AnalysisStage,
    contract_name: &str,
    contract_path: &str,
    source_code: &str,
) -> Result<String> {
    let analysis_summary = serde_json::to_string_pretty(analysis)?;
    let pragma = extract_pragma(source_code);
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
            \"solidity\": \"{pragma}\",\n\
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
        pragma = pragma,
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
                 \"dictionary_weight\": integer\n\
             }}\n\
         }}\n\
         \n\
         Guidance:\n\
         - Choose runs/depth for meaningful state exploration of this contract.\n\
         - seed must be a hex string like \"0xdeadbeef\".\n\
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
    let handler_name = format!("{}Handler", request.contract_name);
    let test_name = format!("{}InvariantTest", request.contract_name);

    Ok(format!(
        "Round: {round}\n\
         Return JSON only. No markdown, no prose, no code fences.\n\
         \n\
         REQUIRED OUTPUT FORMAT:\n\
         {{\n\
           \"mode\": \"patch\",\n\
           \"bodies_updates\": [{{\"op\": \"add|replace|remove\", \"path\": \"string\", \"value\": any, \"reason\": \"string\"}}],\n\
           \"foundry_config_updates\": [{{\"op\": \"add|replace|remove\", \"path\": \"string\", \"value\": any, \"reason\": \"string\"}}]\n\
         }}\n\
         \n\
         PATCH RULES:\n\
         1. Each update item MUST have exactly 4 keys: op, path, value, reason.\n\
         2. path is a dot-path: \"handler.functions.deposit\", \"meta.solidity\", \"depth\".\n\
         3. op must be one of: add (key must not exist), replace (key must exist), remove (set value to null).\n\
         4. No duplicate paths in one response.\n\
         5. If nothing needs changing for one artifact, return its updates array as [].\n\
         \n\
         SOLIDITY CONSTRAINTS (must hold after every patch):\n\
         - contract {handler_name} is Test {{  — do not change this declaration or remove Test inheritance\n\
         - contract {test_name} is Test {{  — do not change this declaration or remove Test inheritance\n\
         - Never redefine functions provided by Test (bound, vm, makeAddr, deal)\n\
         - Never embed raw bytecode — use deployCode(\"Name.sol:Name\") for dependencies\n\
         \n\
         VALID bodies path prefixes:\n\
         - meta.contract / meta.contractPath / meta.solidity / meta.generatedAt\n\
         - handler.contractName / handler.imports / handler.stateVars / handler.ghostVars\n\
         - handler.constructorSignature / handler.constructorBody\n\
         - handler.functions.<functionName>\n\
         - invariantTest.contractName / invariantTest.imports / invariantTest.stateVars\n\
         - invariantTest.setUpBody\n\
         - invariantTest.invariants.<invariantName>\n\
         \n\
         VALID foundry_config path prefixes:\n\
         - depth / runs / seed / max_test_rejects / dictionary_weight\n\
         \n\
         Existing bodies:\n{existing_bodies}\n\
         \n\
         Existing foundry config:\n{existing_config}",
        round = request.round,
        handler_name = handler_name,
        test_name = test_name,
        existing_bodies = existing_bodies_json,
        existing_config = existing_config_json,
    ))
}
