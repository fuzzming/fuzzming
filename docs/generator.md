# Generator Component

The Generator is the **generation engine** of FuzzMing. Each round the orchestrator passes it a `RoundSignal` containing everything it needs — contract source code, fuzz output, coverage gaps, and the previous round's artifacts. It returns an `LlmSignal` carrying the newly generated or updated `BodiesJson` and `FoundryConfig`.

---

## Responsibility

The Generator has one job: given the current state of the fuzzing session, produce valid Solidity handler and invariant test bodies plus a Foundry configuration. It does not run tests, does not read files, and does not store state. Everything it needs arrives in the signal; everything it produces leaves in the signal.

---

## Directory structure

```
src/generator/
├── adapters/
│   ├── inbound/
│   │   └── generator.rs                    # Inbound adapter — implements LlmEnginePort, delegates to GeneratorRunPort
│   └── outbound/
│       ├── litellm_generation_adapter.rs   # Implements GenerationPort — orchestrates 3-stage call chain
│       ├── litellm_client.rs               # Implements LlmClientPort — only file that imports litellm_rs
│       ├── prompt_builder.rs               # Builds stage prompts for the 3-stage chain
│       ├── response_parser.rs              # Extracts, parses, normalizes, repairs JSON responses
│       └── stages.rs                       # AnalysisStage, BodiesStage, ConfigStage types
├── ports/
│   ├── inbound/
│   │   └── generator_run_port.rs           # GeneratorRunPort — inbound contract between adapter and use case
│   └── outbound/
│       ├── generation_port.rs              # GenerationPort + GenerationRequest
│       └── llm_client_port.rs              # LlmClientPort — complete(system, user) -> String
├── use_cases/
│   ├── assemble_prompt.rs                  # Builds AssembledPrompt from RoundSignal context
│   └── run.rs                              # GeneratorRunUseCase — owns outbound ports, implements GeneratorRunPort
└── domain/
    ├── generation_response.rs              # GenerationResponse, GenerationResult, GenerationUsage
    └── prompt.rs                           # Prompt domain type

src/shared/
├── requests/round_signal.rs                # RoundSignal — input from orchestrator
└── responses/llm_signal.rs                 # LlmSignal — output to orchestrator
```

---

## Architecture layers

The component follows hexagonal architecture. The inbound adapter never touches outbound ports directly — it goes through the use case via an inbound port.

```
Orchestrator
    │
    └─ LlmEnginePort (shared/ports)
           │
    Generator (adapters/inbound)                    ← implements LlmEnginePort
           │
    GeneratorRunPort (ports/inbound)                ← inbound contract
           │
    GeneratorRunUseCase (use_cases)                 ← implements GeneratorRunPort, owns outbound ports
           │
    GenerationPort (ports/outbound)                 ← outbound contract
           │
    LiteLlmGenerationAdapter (adapters/outbound)    ← implements GenerationPort
           │
    LlmClientPort (ports/outbound)                  ← outbound contract
           │
    LiteLlmClient (adapters/outbound)               ← implements LlmClientPort, only file calling litellm_rs
```

### Inbound adapter — `adapters/inbound/generator.rs`

Receives `RoundSignal` from the orchestrator via `LlmEnginePort`. Holds `Box<dyn GeneratorRunPort>` — never imports a concrete use case or any outbound type. Delegates entirely to the use case.

### Inbound port — `ports/inbound/generator_run_port.rs`

```rust
pub trait GeneratorRunPort: Send + Sync {
    async fn run(&self, signal: RoundSignal) -> Result<LlmSignal>;
}
```

Defines the contract between the inbound adapter and the use case. Owned by the application core.

### Use case — `use_cases/run.rs`

`GeneratorRunUseCase` implements `GeneratorRunPort`. Owns the outbound dependency:

```rust
pub struct GeneratorRunUseCase {
    gateway: Box<dyn GenerationPort>,
}
```

Contains all the business logic: calls `assemble_prompt` (passing `signal.confirmed_bugs`), builds the `GenerationRequest`, calls the outbound port, maps the response into an `LlmSignal`.

### `assemble_prompt` — `use_cases/assemble_prompt.rs`

```rust
pub fn assemble_prompt(
    round: u32,
    contract_context: ContractContext,
    fuzz_output: Option<String>,
    coverage_context: Option<CoverageContext>,
    confirmed_bugs: Vec<BugInfo>,
) -> Result<AssembledPrompt>
```

Constructs the `Prompt` domain type and calls `into_assembled()`. `confirmed_bugs` is forwarded from `RoundSignal` — it is empty on round 1.

### `Prompt` — `domain/prompt.rs`

`Prompt` builds the system and user messages:

- **System message** — contract source code + five strict operational rules (no for-in loops, physical vs logical, namespacing, IndexMap order, JSON-only output).
- **User message** — assembled from up to four sections in order:
  1. `Round: {n}`
  2. `CONFIRMED BUGS` — rendered only when `confirmed_bugs` is non-empty; lists invariant names the model must not re-generate.
  3. `FUZZ OUTPUT` — rendered only on round ≥ 2.
  4. `COVERAGE GAPS` — rendered only when coverage context is present.
  5. Instruction: full generation (round 1) or patch/rewrite (round N).

`AssembledPrompt.context_sections` records which optional sections were included (`"confirmed_bugs"`, `"fuzz_output"`, `"coverage"`) — used for observability.

### Outbound ports — `ports/outbound/`

| Trait | Purpose |
|---|---|
| `GenerationPort` | What the use case calls — `generate(request) -> GenerationResult` |
| `LlmClientPort` | What the generation adapter calls — `complete(system, user) -> String` |

`LlmClientPort` keeps the `Llm` prefix because it is specifically the contract for an LLM HTTP client — a technology-specific concept, not a domain one.

### Outbound adapters — `adapters/outbound/`

`LiteLlmGenerationAdapter` implements `GenerationPort`. Holds `Box<dyn LlmClientPort>` — never imports `LiteLlmClient` directly.

| File | What it owns |
|---|---|
| `prompt_builder.rs` | Stage-specific user prompts for the 3-stage chain + round-N prompt |
| `response_parser.rs` | JSON extraction, envelope normalization, repair prompts |
| `stages.rs` | `AnalysisStage`, `BodiesStage`, `ConfigStage` — internal types for the chain |

`LiteLlmClient` implements `LlmClientPort`. The only file in the component that imports `litellm_rs`.

---

## Data flow

```
Orchestrator
  │
  └─ Generator::run(RoundSignal)             ← LlmEnginePort (inbound adapter)
       │
       └─ GeneratorRunUseCase::run(signal)   ← GeneratorRunPort (use case)
             │
             ├─ assemble_prompt()
             │     builds system message: contract source + rules
             │     builds user message:   round number + confirmed bugs + fuzz output + coverage gaps
             │     → AssembledPrompt
             │
             ├─ GenerationRequest {
             │       round,
             │       contract_name,    ← from signal (e.g. "Vault")
             │       contract_path,    ← from signal (e.g. "src/Vault.sol")
             │       source_code,
             │       prompt,
             │       existing_bodies,
             │       existing_foundry_config
             │   }
             │
             └─ gateway.generate(request)            ← GenerationPort trait call
                   │
                   └─ LiteLlmGenerationAdapter
                         │
                         ├─ Round 1 → generate_round_one()
                         │     ├─ Stage 1: analysis prompt   → AnalysisStage
                         │     ├─ Stage 2: bodies prompt     → BodiesStage
                         │     └─ Stage 3: config prompt     → ConfigStage
                         │     → GenerationResponse::Full { bodies, foundry_config }
                         │
                         └─ Round N → generate_round_n()
                               → GenerationResponse::Full { ... }
                               or GenerationResponse::Patch { bodies_updates, foundry_config_updates }
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

The prompt tells the LLM exactly:
- The contract names to use (`{Contract}Handler`, `{Contract}InvariantTest`)
- The required import lines (derived by FuzzMing from `contract_name` and `contract_path`)
- The file layout (`test/fuzzming/{Contract}/`)
- That `outputPath` must NOT be included — paths are managed by the tool

Returns `BodiesStage { bodies: BodiesJson }`.

**Stage 3 — Foundry config**

Prompt: given stage 1 analysis and stage 2 function names, generate `FoundryConfig` — runs, depth, seed, call sequence weights.

Returns `ConfigStage { foundry_config: FoundryConfig }`.

---

## Round N — single call

From round 2 onwards, the model receives the assembled prompt (fuzz output + coverage gaps) plus the existing artifacts. It responds with either:

- `Full` — complete replacement of bodies and config
- `Patch` — a list of `JsonBlockUpdate` operations

`Patch` is preferred when only specific functions need to change. Valid patch paths do not include `handler.outputPath` or `invariantTest.outputPath` — those fields no longer exist.

---

## Retry and repair

Each stage uses `MAX_ATTEMPTS = 2`. If the model returns invalid JSON:

1. The parse error and the invalid payload are sent back to the model in a repair prompt
2. The model is asked to fix only the JSON, nothing else
3. After 2 failures the call bails with a clear error

---

## JSON enforcement

`response_format: json_object` is set on every call via `CompletionOptions`.

`extract_json_payload` strips markdown code fences that some models produce despite the format constraint.

---

## Provider configuration

```toml
[llm]
model   = "groq/openai/gpt-oss-120b"
api_key = "gsk_..."
```

The model string encodes the provider via its prefix. The adapter derives the env var name automatically:

```
groq/...        →  GROQ_API_KEY
openrouter/...  →  OPENROUTER_API_KEY
openai/...      →  OPENAI_API_KEY
anthropic/...   →  ANTHROPIC_API_KEY
```

---

## Wiring at startup

```rust
let client    = Box::new(LiteLlmClient::new(model, Some(0.1), Some(4_096)));
let adapter   = Box::new(LiteLlmGenerationAdapter::new(model, api_key, client));
let use_case  = Box::new(GeneratorRunUseCase::new(adapter));
let generator = Generator::new(use_case);
```

`Generator` never imports `GeneratorRunUseCase`. `GeneratorRunUseCase` never imports `LiteLlmGenerationAdapter`. All concrete types are resolved at the entry point only.

---

## Key types

### `GenerationRequest`
```rust
pub struct GenerationRequest {
    pub round: u32,
    pub contract_name: String,           // e.g. "Vault" — used to build prompt constraints
    pub contract_path: String,           // e.g. "src/Vault.sol" — injected into import lines
    pub source_code: String,
    pub prompt: AssembledPrompt,
    pub existing_bodies: Option<BodiesJson>,
    pub existing_foundry_config: Option<FoundryConfig>,
}
```

`contract_name` and `contract_path` are forwarded from `RoundSignal` — the orchestrator derives them from the CLI `--targets` argument, never from the LLM.

### `GenerationResponse`
```rust
pub enum GenerationResponse {
    Full { bodies: BodiesJson, foundry_config: FoundryConfig },
    Patch { bodies_updates: Vec<JsonBlockUpdate>, foundry_config_updates: Vec<JsonBlockUpdate> },
}
```

### `GenerationUsage`
```rust
pub struct GenerationUsage {
    pub calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub reasoning_tokens: u64,
    pub thinking_tokens: u64,
}
```
