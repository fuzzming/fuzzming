# LLM Component

The LLM component is the **generation engine** of FuzzMing. Each round the orchestrator passes it a `RoundSignal` containing everything it needs — contract source code, fuzz output, coverage gaps, and the previous round's artifacts. It returns an `LlmSignal` carrying the newly generated or updated `BodiesJson` and `FoundryConfig`.

---

## Responsibility

The LLM component has one job: given the current state of the fuzzing session, produce valid Solidity handler and invariant test bodies plus a Foundry configuration. It does not run tests, does not read files, and does not store state. Everything it needs arrives in the signal; everything it produces leaves in the signal.

---

## Directory structure

```
src/llm/
├── llm.rs                                  # Controller — wires use cases, calls port
├── ports/
│   ├── generation_port.rs                  # LlmGenerationPort + LlmGenerationRequest/Response
│   ├── llm_client_port.rs                  # LlmClientPort — complete(system, user) -> String
│   └── mod.rs
├── use_cases/
│   ├── assemble_prompt.rs                  # Builds AssembledPrompt from RoundSignal context
│   └── apply_patch.rs                      # Applies JsonBlockUpdate dot-path patches
├── adapters/
│   ├── litellm_generation_adapter.rs       # Implements LlmGenerationPort — orchestrates call chain
│   ├── prompt_builder.rs                   # Builds stage prompts for the 3-stage chain
│   ├── response_parser.rs                  # Extracts, parses, normalizes, repairs JSON responses
│   ├── stages.rs                           # AnalysisStage, BodiesStage, ConfigStage types
│   └── mod.rs
└── infrastructure/
    ├── litellm_client.rs                   # Implements LlmClientPort — only file that imports litellm_rs
    └── mod.rs

src/shared/
├── requests/round_signal.rs                # RoundSignal — input from orchestrator
└── responses/llm_signal.rs                 # LlmSignal — output to orchestrator
```

---

## Architecture layers

The component follows clean architecture strictly. Each layer has one direction of dependency — inward only.

```
interfaces/requests     →   llm.rs (controller)
                                │
                        use_cases/         (pure business logic, no external deps)
                                │
                        ports/             (traits — contracts between layers)
                                │
                        adapters/          (implement ports, use infrastructure via traits)
                                │
                        infrastructure/    (external library calls — litellm_rs only)
```

### Controller — `llm.rs`

Receives `RoundSignal`, calls the two use cases, calls the port, handles the response, returns `LlmSignal`. Does no business logic itself — it is the wiring layer.

### Ports — `ports/`

Two traits:

| Trait | Purpose |
|---|---|
| `LlmGenerationPort` | What `llm.rs` calls — `generate(request) -> response` |
| `LlmClientPort` | What the adapter calls — `complete(system, user) -> String` |

`llm.rs` depends only on `LlmGenerationPort`. The adapter depends only on `LlmClientPort`. Neither imports a concrete class.

### Use cases — `use_cases/`

Pure functions. Zero external dependencies — no `litellm_rs`, no file I/O, no HTTP. They only import from `crate::interfaces`.

| File | What it does |
|---|---|
| `assemble_prompt.rs` | Builds the `AssembledPrompt` — system message (contract source + rules) and user message (round, fuzz output, coverage gaps) |
| `apply_patch.rs` | Walks a dot-path (`handler.functions.handler_deposit`) on a `serde_json::Value` and replaces the value at that key |

### Adapters — `adapters/`

`LiteLlmGenerationAdapter` implements `LlmGenerationPort`. It holds a `Box<dyn LlmClientPort>` — it never imports `LiteLlmClient` directly.

The adapter also owns the translation logic private to the LiteLLM call strategy:

| File | What it owns |
|---|---|
| `prompt_builder.rs` | Stage-specific user prompts for the 3-stage chain + round-N prompt |
| `response_parser.rs` | JSON extraction, envelope normalization, repair prompts |
| `stages.rs` | `AnalysisStage`, `BodiesStage`, `ConfigStage` — internal types for the chain |

These files live in `adapters/` because they exist specifically because of the 3-stage chain strategy. If the strategy changed, they would change — not the use cases.

### Infrastructure — `infrastructure/`

One file: `litellm_client.rs`. It is the only file in the entire component that imports `litellm_rs`. It implements `LlmClientPort` and makes the actual HTTP call.

---

## Data flow

```
Orchestrator
  │
  └─ Llm::run(RoundSignal)
       │
       ├─ assemble_prompt()
       │     builds system message: contract source + 5 rules
       │     builds user message:   round number + fuzz output + coverage gaps
       │     → AssembledPrompt
       │
       ├─ LlmGenerationRequest { round, source_code, prompt, existing_bodies, existing_foundry_config }
       │
       ├─ gateway.generate(request)          ← LlmGenerationPort trait call
       │     │
       │     └─ LiteLlmGenerationAdapter
       │           │
       │           ├─ set_api_key()          derives env var from model prefix (groq/ → GROQ_API_KEY)
       │           │
       │           ├─ Round 1 → generate_round_one()
       │           │     ├─ Stage 1: analysis prompt   → AnalysisStage
       │           │     ├─ (3s delay)
       │           │     ├─ Stage 2: bodies prompt     → BodiesStage
       │           │     ├─ (3s delay)
       │           │     └─ Stage 3: config prompt     → ConfigStage
       │           │     → LlmGenerationResponse::Full { bodies, foundry_config }
       │           │
       │           └─ Round N → generate_round_n()
       │                 → LlmGenerationResponse::Full { ... }
       │                 or LlmGenerationResponse::Patch { bodies_updates, foundry_config_updates }
       │
       ├─ Full response   → use directly
       │
       ├─ Patch response  → apply_bodies_patch() + apply_config_patch()
       │                    walks dot-paths, replaces values, deserializes back to typed structs
       │
       └─ LlmSignal { status: Done, bodies, foundry_config }
```

---

## Round 1 — three chained calls

Round 1 splits into three sequential LLM calls to improve accuracy. Each stage focuses on a narrow task, and each stage feeds its output into the next as context.

**Why three calls instead of one?**
A single prompt asking for security analysis + Solidity generation + Foundry config produces lower quality output. Splitting forces the model to reason about each concern in isolation before combining them.

**Stage 1 — Security analysis**

Prompt: analyze the contract for ghost borrowing, inflation attacks, and rounding errors.

Returns `AnalysisStage`:
```json
{
  "vulnerability_analysis": ["..."],
  "handler_logic_pseudocode": "...",
  "invariant_mathematical_proofs": ["..."],
  "critical_invariants": ["..."]
}
```

**Stage 2 — Solidity generation**

Prompt: given stage 1 analysis, generate the full `BodiesJson` — Handler contract and invariant test contract.

Returns `BodiesStage { bodies: BodiesJson }`.

**Stage 3 — Foundry config**

Prompt: given stage 1 analysis and stage 2 function names, generate `FoundryConfig` — runs, depth, seed, call sequence weights.

Returns `ConfigStage { foundry_config: FoundryConfig }`.

There is a 3-second delay between each stage to avoid rate limits.

---

## Round N — single call

From round 2 onwards, the model receives the assembled prompt (fuzz output + coverage gaps) plus the existing artifacts. It responds with either:

- `Full` — complete replacement of bodies and config
- `Patch` — a list of `JsonBlockUpdate` operations

`Patch` is preferred when only specific functions need to change.

---

## Retry and repair

Each stage uses `MAX_ATTEMPTS = 2`. If the model returns invalid JSON:

1. The parse error and the invalid payload are sent back to the model in a repair prompt
2. The model is asked to fix only the JSON, nothing else
3. After 2 failures the call bails with a clear error

---

## JSON enforcement

`response_format: json_object` is set on every call via `CompletionOptions` — this forces the model to return valid JSON at the API level.

`extract_json_payload` strips markdown code fences (` ```json ... ``` `) that some models produce despite the format constraint.

`normalize_envelope` handles models that wrap their response in a nested object:
```json
{ "mode": "patch", "patch": { "bodies_updates": [...] } }
```
It flattens these into the canonical shape before parsing.

---

## Provider configuration

The user provides two values in their config file:

```toml
[llm]
model   = "groq/openai/gpt-oss-120b"
api_key = "gsk_..."
```

The model string encodes the provider via its prefix. LiteLLM reads the prefix to route the request. The adapter derives the env var name automatically:

```
groq/...        →  GROQ_API_KEY
openrouter/...  →  OPENROUTER_API_KEY
openai/...      →  OPENAI_API_KEY
anthropic/...   →  ANTHROPIC_API_KEY
```

No code change is needed to add a new provider — any prefix LiteLLM supports works automatically.

---

## Wiring at startup

The entry point constructs the dependency chain and injects it:

```rust
let client  = Box::new(LiteLlmClient::new(model, Some(0.1), Some(4_096)));
let adapter = Box::new(LiteLlmGenerationAdapter::new(model, api_key, client));
let llm     = Llm::new(adapter);
```

`llm.rs` never imports `LiteLlmGenerationAdapter`. It only knows `LlmGenerationPort`. The concrete type is resolved at the entry point — nowhere else.

---

## Key types

### `RoundSignal` (`interfaces/requests/`)
```rust
pub struct RoundSignal {
    pub round: u32,
    pub config: SessionConfig,
    pub source_code: String,
    pub fuzz_output: Option<String>,
    pub coverage_context: Option<CoverageContext>,
    pub existing_bodies: Option<BodiesJson>,
    pub existing_foundry_config: Option<FoundryConfig>,
}
```

### `LlmSignal` (`interfaces/responses/`)
```rust
pub struct LlmSignal {
    pub status: LlmStatus,
    pub bodies: Option<BodiesJson>,
    pub foundry_config: Option<FoundryConfig>,
    pub reason: Option<String>,
}
```

### `LlmGenerationResponse`
```rust
pub enum LlmGenerationResponse {
    Full { bodies: BodiesJson, foundry_config: FoundryConfig },
    Patch { bodies_updates: Vec<JsonBlockUpdate>, foundry_config_updates: Vec<JsonBlockUpdate> },
}
```

### `JsonBlockUpdate`
```rust
pub struct JsonBlockUpdate {
    pub path: String,        // dot-path: "handler.functions.handler_deposit"
    pub value: serde_json::Value,
    pub reason: String,
}
```
