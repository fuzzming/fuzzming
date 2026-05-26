use anyhow::{Context, Result};

use crate::generator::ports::outbound::GenerationRequest;
use crate::shared::models::{BodiesJson, PromptMode, Role};

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

/// Parse `import {Sym} from "./rel.sol";` lines in the source and return
/// ready-to-use import strings with paths resolved relative to the contract file.
/// E.g. contract_path="src/EasyBank.sol", source has `import {Token} from "./Token.sol"`
/// → returns `["import {Token} from \"src/Token.sol\";"]`
fn extract_dependency_imports(contract_path: &str, source: &str) -> Vec<String> {
    let dir = contract_path.rfind('/').map_or("", |i| &contract_path[..i]);
    let mut imports = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        if !t.starts_with("import") {
            continue;
        }
        // Match: import {Sym[, Sym2]} from "./rel.sol";
        // Also handles import {Sym} from "../path.sol";
        let from_pos = match t.find("from") {
            Some(p) => p,
            None => continue,
        };
        let symbols = &t[..from_pos]; // "import {Token}"
        let rest = t[from_pos + 4..].trim(); // `"./Token.sol";`
        let path_raw = rest
            .trim_start_matches('"')
            .trim_end_matches(';')
            .trim_end_matches('"');
        if !path_raw.starts_with('.') {
            continue; // skip absolute / lib imports
        }
        // Resolve relative path against the contract's directory
        let resolved = if dir.is_empty() {
            path_raw.trim_start_matches("./").to_string()
        } else {
            let combined = format!("{}/{}", dir, path_raw.trim_start_matches("./"));
            // Normalize simple "../" components
            let mut parts: Vec<&str> = Vec::new();
            for seg in combined.split('/') {
                if seg == ".." {
                    parts.pop();
                } else if seg != "." {
                    parts.push(seg);
                }
            }
            parts.join("/")
        };
        imports.push(format!("{}from \"{}\";", symbols, resolved));
    }
    imports
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

fn is_concise(mode: &PromptMode) -> bool {
    matches!(mode, PromptMode::Concise)
}

pub fn build_round_one_bodies_prompt(
    analysis: &AnalysisStage,
    contract_name: &str,
    contract_path: &str,
    source_code: &str,
    mode: &PromptMode,
) -> Result<String> {
    let analysis_summary = serde_json::to_string_pretty(analysis)?;
    let pragma = extract_pragma(source_code);
    let handler_name = format!("{}Handler", contract_name);
    let test_name = format!("{}InvariantTest", contract_name);

    // Import lines the LLM must use — derived by FuzzMing, not chosen by the LLM.
    let handler_target_import = format!("import {{{}}} from \"{}\";", contract_name, contract_path);
    let test_handler_import = format!(
        "import {{{}}} from \"./{}.sol\";",
        handler_name, handler_name
    );
    let test_std_import = "import {Test} from \"forge-std/Test.sol\";";

    // Dependency imports derived from the contract's own import lines.
    let dep_imports = extract_dependency_imports(contract_path, source_code);
    let dep_imports_block = if dep_imports.is_empty() {
        String::new()
    } else {
        let lines = dep_imports
            .iter()
            .map(|i| format!("    \"{i}\""))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "  If the handler or test must interact with a dependency contract (e.g. call approve, \
transfer, or mint on a token), use EXACTLY these pre-resolved import lines — do not invent paths:\n{lines}\n"
        )
    };

    let rules_block = if is_concise(mode) {
        "STRICT DESIGN RULES:\n\
1. EXTERNAL CALLS ONLY: Handler functions MUST make external calls to the target contract \
instance. Do NOT reimplement the target contract's internal logic inside the handler.\n\
2. NO HALLUCINATIONS: Do not call functions or read variables on the target contract that do \
not explicitly exist in the provided source code.\n\
3. IMPORTS: Use only the import lines listed in REQUIRED IMPORT LINES above. If you need to interact with a dependency (e.g. call approve or transfer on a token), use the pre-resolved import line provided in the dependency imports block — do not invent paths. Never use low-level .call() for a contract whose interface you know.\n\
4. targetSelectors: Always set to empty string \"\". In the invariant test setUp, call `targetContract(address(handler))` — never call `targetSelector(\"\")` or pass a string to targetSelector, it takes a FuzzSelector struct not a string.\n\
5. HANDLER ACCESS FROM INVARIANTS: Public array state vars (e.g. `address[] public actors`) do NOT expose a getActors() method. If the invariant test needs to iterate an array, define a helper in handler.functions using the COMPLETE function syntax, e.g.: `\"actorsLength\": \"function actorsLength() external view returns (uint256) { return actors.length; }\"`. Then call `handler.actorsLength()` from the invariant test. Never call a helper that is not defined in handler.functions.\n\
6. BOUND AMOUNTS TO PREVENT OVERFLOW: Always cap amounts at `type(uint128).max` — never `type(uint256).max`.\n\
7. INITIALIZE GHOST STATE: If the contract is deployed with an initial supply or state, ghost variables MUST be initialized to match in the constructor. Uninitialised ghost state causes false positives from the first invariant check.\n\
8. NO TAUTOLOGICAL INVARIANTS: Never write invariants that are always true by type. `uint256 >= 0` is always true — do not write it. Only write invariants that can actually fail.\n\
9. HANDLER IS THE CALLER: Inside handler functions, ALL calls to the target contract are made FROM `address(this)`, NOT from `msg.sender`. Therefore: (a) ghost mappings must be keyed by `address(this)` not `msg.sender`; (b) use `vm.prank(someAddress)` before the contract call to simulate a specific user; (c) every state-changing call MUST update its corresponding ghost variable.\n\
".to_string()
    } else {
        "STRICT DESIGN RULES:\n\
1. EXTERNAL CALLS ONLY: Handler functions MUST make external calls to the target contract \
instance. Do NOT reimplement the target contract's internal logic inside the handler.\n\
2. NO HALLUCINATIONS: Do not call functions or read variables on the target contract that do \
not explicitly exist in the provided source code.\n\
3. NO REDUNDANCIES: Do not write meaningless checks like `require(myUint >= 0)`.\n\
4. IMPORTS: Use only the import lines listed in REQUIRED IMPORT LINES above. If you need to interact with a dependency (e.g. call approve or transfer on a token), use the pre-resolved import line provided in the dependency imports block — do not invent paths. Never use low-level .call() for a contract whose interface you know.\n\
5. targetSelectors: Always set to empty string \"\". In the invariant test setUp, call `targetContract(address(handler))` — never call `targetSelector(\"\")` or pass a string to targetSelector, it takes a FuzzSelector struct not a string.\n\
6. NO REDEFINING TEST HELPERS: Do not define functions already provided by inheriting Test — never write your own `bound`, `vm`, `makeAddr`, `deal`, or similar.\n\
7. NO RAW BYTECODE: Never embed hex bytecode in setUp. To deploy a contract: use `new ContractName()` if it is imported, `deployCode(\"ContractName.sol:ContractName\")` only for contracts you cannot import.\n\
8. HANDLER ACCESS FROM INVARIANTS: Public array state vars (e.g. `address[] public actors`) do NOT expose a getActors() method. If the invariant test needs to iterate an array, define a helper in handler.functions using the COMPLETE function syntax, e.g.: `\"actorsLength\": \"function actorsLength() external view returns (uint256) { return actors.length; }\"`. Then call `handler.actorsLength()` from the invariant test. Never call a helper that is not defined in handler.functions.\n\
9. ASCII ONLY IN STRINGS: All Solidity string literals must use only plain ASCII characters. Never use Unicode dashes (—, –), smart quotes, or any non-ASCII character. Use plain hyphen (-) or colon (:) instead.\n\
10. NO UNUSED VARIABLES: Never declare a local variable that is not used in the function body — Solidity treats unused variables as compilation errors.\n\
11. BOUND AMOUNTS TO PREVENT OVERFLOW: Always cap amounts at `type(uint128).max` — never `type(uint256).max`.\n\
12. NO DUPLICATE GETTERS: A `public` array (e.g. `address[] public actors`) automatically generates a getter `actors(uint256)`. Never write a separate function with the same name.\n\
13. INTERNAL VS EXTERNAL ACCESS: Inside handler functions use `actors[i]` and `actors.length` directly. The getter syntax only works from outside the contract.\n\
14. ASSERT VS REQUIRE: `assert(condition)` takes exactly one argument. To include a message use `require(condition, \"message\")` — never `assert(condition, \"message\")`.\n\
15. NO MATH LIBRARY: Never use `Math.min()` or `Math.max()`. Compute inline: `a < b ? a : b`.\n\
16. INITIALIZE GHOST STATE: If the contract is deployed with an initial supply or state, ghost variables MUST be initialized to match in the constructor. Uninitialised ghost state causes false positives from the first invariant check.\n\
17. NO TAUTOLOGICAL INVARIANTS: Never write invariants that are always true by type. `uint256 >= 0` is always true — do not write it. Only write invariants that can actually fail.\n\
18. HANDLER IS THE CALLER: Inside handler functions, ALL calls to the target contract are made FROM `address(this)`, NOT from `msg.sender`. Therefore: (a) ghost mappings must be keyed by `address(this)` not `msg.sender`; (b) use `vm.prank(someAddress)` before the contract call to simulate a specific user; (c) every state-changing call MUST update its corresponding ghost variable.\n\
".to_string()
    };

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
  In {test_name}.imports, you MUST include:\n\
    \"{test_handler_import}\"\n\
    \"{test_std_import}\"\n\
    \"{handler_target_import}\"\n\
{dep_imports_block}\
\n\
CONTRACT INHERITANCE (mandatory):\n\
  {handler_name} must inherit from Test: write `contract {handler_name} is Test {{`\n\
  {test_name} must inherit from Test: write `contract {test_name} is Test {{`\n\
\n\
{rules_block}\
\n\
STRICT SCHEMA RULES:\n\
- Use camelCase for all keys.\n\
- Do not combine code into a single field — use the arrays and objects specified below.\n\
- functions and invariants are JSON objects where the value is the COMPLETE function definition as a string — it MUST start with the `function` keyword (include signature, visibility, body). Never put a bare statement or just a return value as the value.\n\
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
            \"stateVars\": [\"ALL state variable declarations including ghost vars, each a full Solidity line ending with ;\"],\n\
            \"ghostVars\": [\"names only of the ghost variables already declared in stateVars, e.g. ghost_balance\"],\n\
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
        dep_imports_block = dep_imports_block,
        analysis_summary = analysis_summary,
        pragma = pragma,
        rules_block = rules_block,
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
         - runs: 256–1000 (start conservative; the tool scales up across rounds).\n\
         - depth: 50–500.\n\
         - seed must be a hex string like \"0xdeadbeef\".\n\
         \n\
         Analysis JSON:\n{}\n\
         \n\
         Handler function names:\n{}",
        analysis_json, functions_json
    ))
}

pub fn build_round_n_prompt(request: &GenerationRequest, mode: &PromptMode) -> Result<String> {
    let existing_bodies_json = serde_json::to_string_pretty(&request.existing_bodies)
        .context("failed to serialize existing bodies")?;
    let existing_config_json = serde_json::to_string_pretty(&request.existing_foundry_config)
        .context("failed to serialize existing foundry config")?;
    let handler_name = format!("{}Handler", request.contract_name);
    let test_name = format!("{}InvariantTest", request.contract_name);

    let patch_constraints = if is_concise(mode) {
        format!(
            "SOLIDITY CONSTRAINTS (must hold after every patch):\n\
             - ALL state variable declarations (including ghost vars) must be in handler.stateVars as full Solidity lines ending with semicolons. handler.ghostVars holds only the variable NAMES (no types, no semicolons) of variables already declared in stateVars.\n\
             - contract {handler_name} is Test  — do not change this declaration or remove Test inheritance.\n\
             - contract {test_name} is Test  — do not change this declaration or remove Test inheritance.\n\
             - To iterate actors in an invariant, use handler.actorsLength() and handler.actors(i) — never call handler.getActors().\n\
             - HANDLER IS THE CALLER: Ghost mappings must be keyed by address(this) not msg.sender. Every state-changing call must update its ghost variable.\n\
             "
        )
    } else {
        format!(
            "SOLIDITY CONSTRAINTS (must hold after every patch):\n\
             - ALL state variable declarations (including ghost vars) must be in handler.stateVars as full Solidity lines ending with semicolons. handler.ghostVars holds only the variable NAMES (no types, no semicolons) of variables already declared in stateVars.\n\
             - contract {handler_name} is Test  — do not change this declaration or remove Test inheritance.\n\
             - contract {test_name} is Test  — do not change this declaration or remove Test inheritance.\n\
             - Never redefine functions provided by Test (bound, vm, makeAddr, deal).\n\
             - Never embed raw bytecode — use deployCode(\"Name.sol:Name\") for dependencies.\n\
             - To iterate actors in an invariant, use handler.actorsLength() and handler.actors(i) — never call handler.getActors().\n\
             - ASCII ONLY IN STRINGS: All string literals must use only plain ASCII. No Unicode dashes or smart quotes.\n\
             - PUBLIC ARRAY GETTERS: A `public` array already generates a getter automatically. Never write a separate function with the same name.\n\
             - IMPORT PATHS: Import dependencies from their own source file — never re-export them from the target contract file.\n\
             - HANDLER IS THE CALLER: Ghost mappings must be keyed by address(this) not msg.sender. Every state-changing call must update its ghost variable.\n\
             - ASSERT VS REQUIRE: assert(condition) takes one argument only. Use require(condition, \"msg\") for messages.\n\
             "
        )
    };

    Ok(format!(
        "Round: {round}\n\
         Return JSON only. No markdown, no prose, no code fences.\n\
         \n\
         REQUIRED OUTPUT FORMAT — YOU MUST USE EXACTLY THIS SHAPE:\n\
         {{\n\
           \"mode\": \"patch\",\n\
           \"bodies_updates\": [{{\"op\": \"add|replace|remove\", \"path\": \"string\", \"value\": any, \"reason\": \"string\"}}],\n\
           \"foundry_config_updates\": [{{\"op\": \"add|replace|remove\", \"path\": \"string\", \"value\": any, \"reason\": \"string\"}}]\n\
         }}\n\
         \n\
         CRITICAL: mode MUST be \"patch\". NEVER return \"mode\": \"full\" in round N — it will fail to parse.\n\
         CRITICAL: The top-level keys must be exactly: mode, bodies_updates, foundry_config_updates. No other keys.\n\
         \n\
         PATCH RULES:\n\
         1. Each update item MUST have exactly 4 keys: op, path, value, reason.\n\
         2. path is a dot-path: \"handler.functions.deposit\", \"meta.solidity\", \"depth\".\n\
         3. op must be one of: add (key must not exist), replace (key must exist), remove (set value to null).\n\
         4. No duplicate paths in one response.\n\
         5. If nothing needs changing for one artifact, return its updates array as [].\n\
         \n\
         {patch_constraints}\
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
         - depth (50–500) / runs (256–1000) / seed / max_test_rejects / dictionary_weight\n\
         \n\
         Existing bodies:\n{existing_bodies}\n\
         \n\
         Existing foundry config:\n{existing_config}",
        round = request.round,
        existing_bodies = existing_bodies_json,
        existing_config = existing_config_json,
        patch_constraints = patch_constraints,
    ))
}
