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
     - Reset/clear completeness: For every function named reset*, clear*, delete*, or disable*, \
       list ALL fields in the affected struct or mapping. For each field NOT modified, explicitly \
       state whether the omission is intentional or a bug (e.g. a missing baseFee clear after \
       resetDynamicFee). A field that logically belongs to the same configuration unit but is \
       silently retained after a reset is a Medium/Low severity finding.\n\
     - tx.origin usage: Search the source for any use of tx.origin. If found, list every code \
       path affected. Note that tx.origin-dependent paths CANNOT be tested from view invariant \
       functions — they require vm.prank(actor, actor) in a handler function plus ghost state \
       recording. Flag this explicitly so the generator uses the ghost pattern for those paths.\n\
     \n\
     Discovery objective: find many independent root causes, not many ways to trigger \
     the same defect. Group equivalent exploit paths under one finding when they come \
     from the same code problem. The generator should later use this analysis to create \
     tests for different possible problems, not duplicate invariants for the same problem.\n\
     \n\
     Return this JSON exactly:\n\
     {\n\
       \"vulnerability_analysis\": [\"string — one entry per finding\"],\n\
       \"handler_logic_pseudocode\": \"string describing what state the handler must track\",\n\
       \"invariant_mathematical_proofs\": [\"string — one entry per invariant\"],\n\
       \"critical_invariants\": [\"string\"],\n\
       \"tx_origin_paths\": [\"string — one entry per code path that reads tx.origin. Each entry must name: (1) the function, (2) what tx.origin gates, (3) the ghost variable name to store the result, (4) the invariant to assert on that ghost. Leave empty array [] if tx.origin is not used.\"]\n\
     }"
    .to_string()
}

/// Parse `import {Sym} from "./rel.sol";` lines in the source and return
/// ready-to-use import strings with paths resolved relative to the contract file.
/// E.g. contract_path="src/EasyBank.sol", source has `import {Token} from "./Token.sol"`
/// → returns `["import {Token} from \"src/Token.sol\";"]`
fn resolve_relative_path(dir: &str, raw: &str) -> String {
    let combined = format!("{}/{}", dir, raw.trim_start_matches("./"));
    let mut parts: Vec<&str> = Vec::new();
    for seg in combined.split('/') {
        if seg == ".." {
            parts.pop();
        } else if seg != "." {
            parts.push(seg);
        }
    }
    parts.join("/")
}

fn extract_dependency_imports(contract_path: &str, source: &str) -> Vec<String> {
    let dir = contract_path.rfind('/').map_or("", |i| &contract_path[..i]);
    let mut imports = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        if !t.starts_with("import") {
            continue;
        }

        if let Some(from_pos) = t.find(" from ").or_else(|| t.find("\tfrom ")) {
            // Named import: import {Foo} from "./foo.sol";
            let symbols = &t[..from_pos + 1]; // include the space before "from"
            let rest = t[from_pos + 6..].trim(); // skip " from "
            let path_raw = rest
                .trim_start_matches('"')
                .trim_end_matches(';')
                .trim_end_matches('"');
            if !path_raw.starts_with('.') {
                continue;
            }
            let resolved = if dir.is_empty() {
                path_raw.trim_start_matches("./").to_string()
            } else {
                resolve_relative_path(dir, path_raw)
            };
            let resolved = to_import_path(&resolved).to_string();
            imports.push(format!("{symbols}from \"{resolved}\";"));
        } else {
            // Bare path import: import "../foo.sol";
            let inner = t
                .trim_start_matches("import")
                .trim()
                .trim_start_matches('"')
                .trim_end_matches(';')
                .trim_end_matches('"');
            if !inner.starts_with('.') {
                continue;
            }
            let resolved = if dir.is_empty() {
                inner.trim_start_matches("./").to_string()
            } else {
                resolve_relative_path(dir, inner)
            };
            let resolved = to_import_path(&resolved).to_string();
            imports.push(format!("import \"{resolved}\";"));
        }
    }
    imports
}

fn is_concise(mode: &PromptMode) -> bool {
    matches!(mode, PromptMode::Concise)
}

/// If `contract_path` is an absolute path, strip everything up to the first
/// standard Solidity source directory so the generated import works from the
/// Foundry workspace root (e.g. `contracts/`, `src/`, `test/`).
fn to_import_path(contract_path: &str) -> &str {
    for marker in &["contracts/", "src/", "test/"] {
        if let Some(pos) = contract_path.find(marker) {
            return &contract_path[pos..];
        }
    }
    contract_path
}

pub fn build_round_one_bodies_prompt(
    analysis: &AnalysisStage,
    contract_name: &str,
    contract_path: &str,
    source_code: &str,
    mode: &PromptMode,
) -> Result<String> {
    let analysis_summary = serde_json::to_string_pretty(analysis)?;
    let handler_name = format!("{contract_name}Handler");
    let test_name = format!("{contract_name}InvariantTest");

    let import_path = to_import_path(contract_path);
    let handler_target_import = format!("import {{{contract_name}}} from \"{import_path}\";");
    let test_handler_import = format!("import {{{handler_name}}} from \"./{handler_name}.sol\";");
    let test_std_import = "import {Test} from \"forge-std/Test.sol\";";

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
1. EXTERNAL CALLS ONLY: Handler functions MUST call the target contract — never reimplement its logic. \
(a) NEVER import or use internal libraries from the target (FullMath, SafeMath, TickMath…); \
(b) NEVER copy formulas from the source; \
(c) Call public view functions like `target.getFee(pool)` and store the result.\n\
2. NO HALLUCINATIONS: Only call functions and read variables that explicitly exist in the provided source.\n\
3. IMPORTS: Use only the pre-resolved import lines listed in REQUIRED IMPORT LINES. Never invent paths or use low-level .call() for a contract whose interface you know.\n\
4. setUp REGISTRATION: In setUpBody call ONLY `targetContract(address(handler))`. Never call any targetSelector variant — it takes a FuzzSelector struct, not a string. Set the targetSelectors JSON field to \"\".\n\
5. HANDLER ACCESS FROM INVARIANTS: The invariant test has NO direct access to handler state variables. Every handler array or mapping must be accessed via a public getter prefixed with `handler.` — e.g. `handler.actorsLength()`, `handler.actors(i)`, `handler.poolsLength()`, `handler.pools(i)`. Never write `pools.length` or `pools[i]` bare in an invariant function — `pools` is not declared in that scope. If you need to iterate a handler array, define a getter in handler.functions first.\n\
6. GHOST STATE — INITIALIZE: Ghost variables MUST be initialized in the constructor to match the contract's deployed state. Uninitialised ghost state produces false positives from the very first invariant check.\n\
7. GHOST STATE — SYNC ON ACTOR REGISTRATION: When registering a new actor, immediately read its current balance/state from the contract and assign the ghost variable. Never assume zero.\n\
8. INVARIANTS MUST HOLD AT T=0: Every invariant must pass immediately after setUp(), before any handler call. Verify logic against the initial deployed state.\n\
9. NO TAUTOLOGICAL INVARIANTS: Only write invariants that can actually fail — `uint256 >= 0` is always true, do not write it.\n\
10. HANDLER IS THE CALLER: All target calls are FROM `address(this)`, not `msg.sender`. Key ghost mappings by `address(this)`. Use `vm.prank(actor)` to simulate users. Every state-changing call must update its ghost variable.\n\
11. PREVENT DUPLICATE ACTORS: Guard addActor with `mapping(address => bool) public ghost_isActor`. At the top: `if (ghost_isActor[actor]) return; ghost_isActor[actor] = true;`. Declare addActor as `public` (not `external`) so handler functions can call it internally.\n\
12. BOUND NEEDS UINT256: `bound(x, min, max)` requires `uint256`. Cast first: `uint8(bound(uint256(myUint8), 0, 255))`. Cap amounts at `type(uint128).max`, never `type(uint256).max`.\n\
13. CAST BEFORE ASSERT: `assertEq`, `assertLe`, `assertGe`, `assertLt`, `assertGt` only have overloads for `uint256`, `int256`, `bool`, and `address` in Solidity 0.7.x. Always cast smaller types first: `assertEq(uint256(myUint24), 0, ...)`. Never call these with `uint24`, `uint128`, `int24`, etc. directly.\n\
14. AVOID STACK-TOO-DEEP (Solidity 0.7.x): Each function body must declare at most 4 local variables. Split complex invariants into multiple small functions — one property each. Never destructure more than 2 tuple values at once.\n\
15. COUNT TUPLE FIELDS EXACTLY: When destructuring a public mapping that returns a struct (e.g. `(a, b, c) = target.myMapping(key)`), count the exact number of fields in the struct definition in the source code and use exactly that many slots. Never guess — an off-by-one causes a compile error.\n\
16. NO CHEAT CODES IN INVARIANTS: Invariant functions are `view` — never call `vm.prank`, `vm.warp`, `vm.roll`, `vm.deal`, or any other cheat code inside them. State setup belongs in handler functions, not invariant checks.\n\
17. MOCK EXTERNAL DEPENDENCIES: If the target constructor takes an address it later calls, write a minimal mock in `handler.helperContracts` — a full `contract MockDep { ... }` string placed before the Handler in the same file. Scan the source for every call the target makes on that dependency and implement only those functions. Deploy in constructorBody: `MockDep mock = new MockDep(); target = new Target(address(mock), ...);`. Never import a mock from a separate file; never cast raw addresses to contract types.\n\
18. ACTORS ARRAY IS THE ONLY SOURCE OF ADDRESSES: Every address the target interacts with MUST be deployed and pushed into `actors` in the constructor. In handler functions pick with `actors[seed % actors.length]`. Never derive addresses via keccak256 or uint160 casts — they are unregistered and every call using them reverts silently.\n\
19. ASCII ONLY IN STRINGS: All Solidity string literals must use only plain ASCII characters (codes 0-127). Never use Unicode dashes, smart quotes, ellipsis, or any non-ASCII glyph. Use plain hyphen (-) or colon (:) instead. One non-ASCII character in a string causes a parse error and wastes a full round.\n\
20. MATCH REQUIRE BOUNDS EXACTLY: For every handler function that calls a target function, read ALL require statements in that target function and enforce the exact same constraints in your bound() calls. Copy the exact constants — if the target has require(_fee <= MAX_BASE_FEE || _fee == ZERO_FEE_INDICATOR), the handler must bound to [0, MAX_BASE_FEE] or explicitly use ZERO_FEE_INDICATOR. A handler that lets values outside the target require range through causes false-positive invariant failures.\n\
21. TX.ORIGIN PATTERN: If the target reads tx.origin anywhere in its source, that path CANNOT be tested from a view invariant function. Follow this pattern exactly — no variations:\n\
    (a) Declare a ghost variable in stateVars to store the result: `uint256 public ghost_feeForDiscountedActor;`\n\
    (b) Write a handler function that: registers the actor as discounted, calls vm.prank(actor, actor) — the TWO-argument form sets BOTH msg.sender AND tx.origin — then calls the target and saves the result to the ghost variable:\n\
        `function handle_getFeeAsDiscountedActor(uint256 actorSeed) external {`\n\
        `    address actor = actors[actorSeed % actors.length];`\n\
        `    target.registerDiscounted(actor, 100000);`\n\
        `    vm.prank(actor, actor);`\n\
        `    ghost_feeForDiscountedActor = target.getFee(pool);`\n\
        `}`\n\
    (c) Write an invariant that reads ONLY the ghost variable — never call the target directly from an invariant when tx.origin matters:\n\
        `function invariant_discountedFeeNeverExceedsFullFee() external view {`\n\
        `    assertLe(uint256(ghost_feeForDiscountedActor), uint256(target.MAX_FEE_CAP()), \"discounted fee exceeds cap\");`\n\
        `}`\n\
    Never call a tx.origin-dependent function directly from inside an invariant — tx.origin will always be the test contract address, the discounted mapping will always return 0, and the bug will never fire.\n\
22. FUZZABLE MOCK STATE: Mock contracts must have mutable state, not hardcoded return values. For every value the mock returns that the target branches on (e.g. currentTick, observationCardinality, lastObsTimestamp), add a public state variable and a setter. Write handler functions (e.g. handle_setMockCurrentTick) that fuzz those values with bounded inputs. A static mock makes entire code branches invisible to the fuzzer.\n\
23. PATCH CONSTRUCTORBODY REQUIRES PATCHING STATEVARS: When a patch round adds or changes any variable assignment in constructorBody (e.g. `swapFeeManager = address(this)`, `mockFactory = new MockFactory()`), you MUST also add the corresponding state variable declaration to stateVars in the same patch. Every variable ASSIGNED in constructorBody that is not DECLARED in stateVars causes an \"Undeclared identifier\" compile error that wastes the entire round. Rule: for every new name on the left-hand side of an assignment in constructorBody, there must be a matching `type public name;` line in stateVars.\n\
".to_string()
    } else {
        "STRICT DESIGN RULES:\n\
1. EXTERNAL CALLS ONLY: Handler functions MUST make external calls to the target contract \
instance. Do NOT reimplement the target contract's internal logic inside the handler. \
This means: (a) NEVER import or use any internal library from the target's source (e.g. FullMath, SafeMath, TickMath) — call the target's public getter/view functions instead; \
(b) NEVER copy formulas or math from the target's source into the handler; \
(c) If the target has a public view function like `getFee(pool)`, call `target.getFee(pool)` and store the result — do NOT re-derive the fee yourself.\n\
2. NO HALLUCINATIONS: Do not call functions or read variables on the target contract that do \
not explicitly exist in the provided source code.\n\
3. NO REDUNDANCIES: Do not write meaningless checks like `require(myUint >= 0)`.\n\
4. IMPORTS: Use only the import lines listed in REQUIRED IMPORT LINES above. If you need to interact with a dependency (e.g. call approve or transfer on a token), use the pre-resolved import line provided in the dependency imports block — do not invent paths. Never use low-level .call() for a contract whose interface you know.\n\
5. targetSelectors: Always set to empty string \"\". In the invariant test setUp, call `targetContract(address(handler))` — never call `targetSelector(\"\")` or pass a string to targetSelector, it takes a FuzzSelector struct not a string. Never emit any line that contains `targetSelector` or `targetSelectors` inside setUpBody.\n\
6. NO REDEFINING TEST HELPERS: Do not define functions already provided by inheriting Test — never write your own `bound`, `vm`, `makeAddr`, `deal`, or similar.\n\
7. NO RAW BYTECODE: Never embed hex bytecode in setUp. To deploy a contract: use `new ContractName()` if it is imported, `deployCode(\"ContractName.sol:ContractName\")` only for contracts you cannot import.\n\
8. HANDLER ACCESS FROM INVARIANTS: Public array state vars (e.g. `address[] public actors`) do NOT expose a getActors() method. If the invariant test needs to iterate an array, define a helper in handler.functions using the COMPLETE function syntax, e.g.: `\"actorsLength\": \"function actorsLength() external view returns (uint256) { return actors.length; }\"`. Then call `handler.actorsLength()` from the invariant test. Never call a helper that is not defined in handler.functions.\n\
9. ASCII ONLY IN STRINGS: All Solidity string literals must use only plain ASCII characters. Never use Unicode dashes (—, –), smart quotes, or any non-ASCII character. Use plain hyphen (-) or colon (:) instead.\n\
10. NO UNUSED VARIABLES: Never declare a local variable that is not used in the function body — Solidity treats unused variables as compilation errors.\n\
11. BOUND AMOUNTS TO PREVENT OVERFLOW: Always cap amounts at `type(uint128).max` — never `type(uint256).max`.\n\
12. NO DUPLICATE GETTERS: A `public` state variable automatically generates a getter with that exact name. NEVER define a function whose name matches any public state variable — e.g. if `EtherVault public target;` is declared, do NOT write `function target() ...`. This causes a \"Identifier already declared\" compile error.\n\
13. INTERNAL VS EXTERNAL ACCESS: Inside handler functions use `actors[i]` and `actors.length` directly. The getter syntax only works from outside the contract.\n\
14. ASSERT VS REQUIRE: `assert(condition)` takes exactly one argument. To include a message use `require(condition, \"message\")` — never `assert(condition, \"message\")`.\n\
15. NO LOCAL VARIABLE SHADOWING: Never declare a local variable with the same name as a function or state variable in the same contract. For example, if the handler declares `function deposit(...)`, do NOT write `uint256 deposit = ...;` inside any function body — name it `amount`, `depositAmount`, or similar instead. Solidity raises a compile error on shadowing.\n\
16. NO MATH LIBRARY: Never use `Math.min()` or `Math.max()`. Compute inline: `a < b ? a : b`.\n\
17. INITIALIZE GHOST STATE: If the contract is deployed with an initial supply or state, ghost variables MUST be initialized to match in the constructor. Uninitialised ghost state causes false positives from the first invariant check.\n\
18. SYNC GHOST STATE ON ACTOR REGISTRATION: When an addActor (or equivalent) function registers a new actor, immediately read that actor's current balance/state from the contract and assign it to the ghost variable. Never assume a newly registered actor has zero balance — they may already hold tokens.\n\
19. INVARIANTS MUST HOLD AT T=0: Every invariant must be true immediately after setUp() completes, before any handler calls. If an invariant can fail on the initial state it is a false positive, not a real bug. Verify your invariant logic against the deployed initial state.\n\
20. NO TAUTOLOGICAL INVARIANTS: Never write invariants that are always true by type. `uint256 >= 0` is always true — do not write it. Only write invariants that can actually fail.\n\
21. HANDLER IS THE CALLER: Inside handler functions, ALL calls to the target contract are made FROM `address(this)`, NOT from `msg.sender`. Therefore: (a) ghost mappings must be keyed by `address(this)` not `msg.sender`; (b) use `vm.prank(someAddress)` before the contract call to simulate a specific user; (c) every state-changing call MUST update its corresponding ghost variable.\n\
22. ACCESS CONSTANTS VIA INSTANCE NOT TYPE: Never write `ContractName.CONSTANT_NAME` or `ContractName.CONSTANT_NAME()` — Solidity does not allow accessing public constants or getters via the contract type name. Always access them via an instance variable, e.g. `target.CONSTANT_NAME()`.\n\
23. STRUCT ACCESS FROM EXTERNAL CONTRACTS: When a target contract defines a struct INSIDE its body (e.g. `contract C { struct S {...} }`), reference it from the handler as `ContractName.StructName` — NEVER as bare `StructName`. Example: `VestingWallet.Schedule memory s = target.schedules(key);` — NOT `Schedule memory s = target.schedules(key);`. If assigning to the struct fails, fall back to tuple destructuring: `(uint256 a, uint256 b, ) = target.schedules(key);`. Do NOT call `.fieldName` directly on the getter return without assigning to a variable first.\n\
24. PREVENT DUPLICATE ACTORS: In addActor (or any actor-registration function), guard against re-registration. Declare `mapping(address => bool) public ghost_isActor;` in stateVars. At the top of addActor: `if (ghost_isActor[actor]) return; ghost_isActor[actor] = true;`. Duplicate actors cause invariants that sum over the array to double-count balances, producing false positives.\n\
25. addActor MUST BE PUBLIC: Declare `addActor` as `public`, not `external`. If other handler functions call `addActor` internally (e.g. inside `transfer` to auto-register participants), `external` will cause a compile error. Always use `public`.\n\
26. BOUND NEEDS UINT256: `bound(x, min, max)` requires `x` to be `uint256`. Never pass `address` or `msg.sender` directly — cast first: `bound(uint256(uint160(msg.sender)), 0, max)`.\n\
27. AVOID STACK-TOO-DEEP (Solidity 0.7.x): Each function body must declare at most 4 local variables. Split complex invariants into multiple small functions — one property each. Never destructure more than 2 tuple values at once.\n\
28. MOCK EXTERNAL DEPENDENCIES: If the target constructor accepts an address it later calls functions on, write a minimal mock using `handler.helperContracts`. Steps: (1) scan the source for every call the target makes on that dependency (e.g. `factory.swapFeeManager()`, `factory.isPool(pool)`); (2) write the full `contract MockDep { ... }` definition as a single string in `handler.helperContracts` — it is placed before the Handler in the same file, so no import is needed and no separate .sol file should be created; (3) add `MockDep public mock;` to stateVars; (4) in constructorBody: `mock = new MockDep(); target = new TargetContract(address(mock), ...);`. NEVER import a mock from a separate file. NEVER cast a raw address like `address(0x123)`, `address(this)`, or `address(handler)` to a contract type — those addresses hold no code and every call silently reverts.\n\
29. ACTORS ARRAY IS THE ONLY SOURCE OF ADDRESSES: Any address the target contract will interact with (pools, users, tokens) MUST be deployed or registered in the constructor and immediately pushed into `actors`. In handler functions, always select addresses using `actors[seed % actors.length]` — NEVER generate addresses with `keccak256`, `address(uint160(...))`, or any other derivation. An address not in `actors` was never registered with mock dependencies and every target call using it will revert silently, making the entire fuzz run useless.\n\
30. PATCH CONSTRUCTORBODY REQUIRES PATCHING STATEVARS: When a patch round adds or changes any variable assignment in constructorBody (e.g. `swapFeeManager = address(this)`, `mockFactory = new MockFactory()`), you MUST also add the corresponding state variable declaration to stateVars in the same patch. Every variable ASSIGNED in constructorBody that is not DECLARED in stateVars causes an \"Undeclared identifier\" compile error that wastes the entire round. Rule: for every new name on the left-hand side of an assignment in constructorBody, there must be a matching `type public name;` line in stateVars.\n\
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
            \"generatedAt\": \"timestamp\"\n\
        }},\n\
        // NOTE: meta.solidity is set automatically from the source file — do not include it.\n\
        \"handler\": {{\n\
            \"contractName\": \"{handler_name}\",\n\
            \"imports\": [\"array of import lines\"],\n\
            \"helperContracts\": [\"optional — full contract definitions for mocks/helpers placed before the Handler in the same file; omit or leave empty if not needed\"],\n\
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
         Analysis JSON:\n{analysis_json}\n\
         \n\
         Handler function names:\n{functions_json}"
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
             - EXTERNAL CALLS ONLY: Never import or use internal libraries from the target's source (e.g. FullMath, SafeMath, TickMath). Call the target's public getter functions instead — e.g. `target.getFee(pool)` not a reimplemented formula.\n\
             - ACTORS ARRAY IS THE ONLY SOURCE OF ADDRESSES: handler functions must pick addresses with `actors[seed % actors.length]`. Never use keccak256 or address(uint160(...)) to derive addresses — they are never registered and every call using them reverts.\n\
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
             - MOCK EXTERNAL DEPENDENCIES: If the target constructor takes an address it calls functions on, deploy a mock: `contract MockDep {{...}}`, then `mock = new MockDep(); target = new TargetContract(address(mock), ...);`. Never cast raw addresses like address(0x123) or address(this) to a contract type — they have no code.\n\
             - EXTERNAL CALLS ONLY: Never import or use internal libraries from the target's source (e.g. FullMath, SafeMath, TickMath). Call the target's public getter functions instead — e.g. `target.getFee(pool)` not a reimplemented formula.\n\
             - ACTORS ARRAY IS THE ONLY SOURCE OF ADDRESSES: handler functions must pick addresses with `actors[seed % actors.length]`. Never use keccak256 or address(uint160(...)) to derive addresses — they are never registered and every call using them reverts.\n\
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
         6. New invariants should target distinct root causes. Use the security analysis to \
         understand confirmed failures, then move to other possible problems. Do not add \
         another invariant whose only purpose is to rediscover a confirmed vulnerability \
         through a different call sequence or symptom.\n\
         \n\
         {patch_constraints}\
         \n\
         VALID bodies path prefixes:\n\
         - meta.contract / meta.contractPath / meta.generatedAt  (meta.solidity is read-only, set automatically)\n\
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
