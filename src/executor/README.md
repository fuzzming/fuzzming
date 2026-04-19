# Executor

The Executor is the **single write gateway** for FuzzMing. It is the only component allowed to write files to disk. It never reads. It never inspects developer-owned files.

---

## Table of Contents

- [Role in the architecture](#role-in-the-architecture)
- [Internal structure](#internal-structure)
- [The two axes](#the-two-axes)
- [Data flow](#data-flow)
- [File ownership](#file-ownership)
- [Artifacts](#artifacts)
- [ExecutorPort](#executorport)
- [Constraints](#constraints)

---

## Role in the architecture

```
SessionOrchestrator
        │
        ├── llm_engine.run(signal) ──► LlmSignal { executor_input: ExecutorInput }
        │                                                │
        └── executor.execute(input) ◄───────────────────┘
                │
                ├── use_cases::write_bodies       → test/<Contract>.bodies.json
                ├── adapters::SolidityGenerator   → test/handlers/<Contract>Handler.sol
                │                                 → test/invariants/<Contract>InvariantTest.sol
                └── adapters::FoundryConfigWriter → foundry.toml
```

`ExecutorPort` is defined in `src/interfaces/ports/executor_port.rs` — the Orchestrator holds it directly and calls it after the LLM round completes. The LLMEngine does not call the Executor.

---

## Internal structure

```
executor/
├── executor.rs               ← implements ExecutorPort, sequences the three writes
├── use_cases/
│   └── write_bodies.rs       ← serialises BodiesJson to test/<Contract>.bodies.json
├── ports/
│   ├── code_generator_port.rs  ← language axis trait
│   └── config_writer_port.rs   ← fuzzer axis trait
├── adapters/
│   ├── solidity_generator.rs   ← implements CodeGeneratorPort for Solidity
│   └── foundry_config_writer.rs ← implements ConfigWriterPort for Foundry
└── infrastructure/
    └── file_system_writer.rs   ← the only code that calls tokio::fs::write
```

---

## The two axes

The Executor is the intersection of two independent extension axes. Each is behind a port injected at composition time.

**Language axis — `CodeGeneratorPort`**

```rust
pub trait CodeGeneratorPort: Send + Sync {
    async fn generate(&self, bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()>;
}
```

Stateless. Receives `BodiesJson` per call. Knows the structure of the target language. Current implementation: `SolidityGenerator`. Future: `RustGenerator`, `VyperGenerator`.

**Fuzzer axis — `ConfigWriterPort`**

```rust
pub trait ConfigWriterPort: Send + Sync {
    async fn write(&self, config: &FuzzerConfigArtifact, writer: &FileSystemWriter) -> Result<()>;
}
```

Stateless. Receives `FuzzerConfigArtifact` per call. Knows the config format of the target fuzzer. Current implementation: `FoundryConfigWriter`. Future: `EchidnaConfigWriter`, `MedusaConfigWriter`.

Both are injected by `CompositionRoot` based on `SessionConfig.language` and `SessionConfig.fuzzer`. Adding a new language or fuzzer means adding one adapter — nothing else changes.

---

## Data flow

```
LLMEngine produces ExecutorInput { bodies: BodiesJson, fuzzer_config: FuzzerConfigArtifact }
        │
        ▼
Executor.execute(input)
        │
        ├── write_bodies(&input.bodies, &self.writer)
        │       path    = "test/{bodies.meta.contract}.bodies.json"
        │       content = serde_json::to_string_pretty(&bodies)
        │       writer.write_file(path, content)
        │
        ├── self.generator.generate(&input.bodies, &self.writer)
        │       SolidityGenerator:
        │         generate_handler     → writer.write_file(handler.output_path, .sol)
        │         generate_invariant_test → writer.write_file(invariant_test.output_path, .sol)
        │
        └── self.config_writer.write(&input.fuzzer_config, &self.writer)
                FoundryConfigWriter:
                  match FuzzerConfigArtifact::Foundry(c)
                  build_fuzzming_section(c)   → patch [profile.fuzzming]
                  build_coverage_section()    → patch [profile.coverage] (round 1 only)
                  writer.write_file("foundry.toml", toml)
```

The three writes always run in this order. If any step fails the error propagates immediately — no partial recovery.

---

## File ownership

| File | Written by | Trigger |
|---|---|---|
| `test/<Contract>.bodies.json` | `write_bodies` | Every round |
| `test/handlers/<Contract>Handler.sol` | `SolidityGenerator` | Every round, after bodies.json |
| `test/invariants/<Contract>InvariantTest.sol` | `SolidityGenerator` | Every round, after bodies.json |
| `foundry.toml` | `FoundryConfigWriter` | Every round |

**Never touched:**

| File | Reason |
|---|---|
| `test/*.t.sol` | Developer-owned — read-only context for LLMEngine |
| `test/*.invariants.t.sol` | Developer-owned — read-only context for LLMEngine |

---

## Artifacts

**`ExecutorInput`**

```
ExecutorInput
├── bodies: BodiesJson
│     meta.contract      → drives file path for bodies.json
│     handler.*          → drives Handler.sol assembly
│     invariant_test.*   → drives InvariantTest.sol assembly
│
└── fuzzer_config: FuzzerConfigArtifact
      Foundry(FoundryConfig)
        depth, runs, seed, max_test_rejects
        dictionary_weight, call_sequence_weights
        current_toml: Option<String>   ← current foundry.toml, read by Reader, never by Executor
```

**`FuzzerConfigArtifact`**

Enum that wraps the config type for each supported fuzzer. Adding a new fuzzer adds one variant. The `ConfigWriterPort` impl matches on it to extract the concrete config.

---

## ExecutorPort

```rust
// src/llm/ports/executor_port.rs
#[async_trait]
pub trait ExecutorPort: Send + Sync {
    async fn execute(&self, input: ExecutorInput) -> Result<()>;
}
```

One method. The LLMEngine calls it once per round after generating its output. The Orchestrator never calls it.

---

## Constraints

- `tokio::fs::write` is called only in `FileSystemWriter::write_file`. No adapter calls it directly.
- All paths are relative to the workspace root held by `FileSystemWriter`.
- The Executor never reads. `FoundryConfig.current_toml` is read by the Reader and forwarded through the artifact — the Executor only patches and writes.
- The Executor holds no mutable state. Every call is a pure function of its inputs.
