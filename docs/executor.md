# Executor Component

The Executor is the **write gateway** of FuzzMing. After the Generator produces new bodies and config, the orchestrator packages them into an `ExecutorInput` and calls the Executor. The Executor writes the generated Solidity files and patches `foundry.toml`. It never reads files and never runs external processes.

---

## Responsibility

One job: take `BodiesJson` and `FuzzerConfigArtifact` from `ExecutorInput` and write them to disk. All logic for how to write is inside the use case: the inbound adapter is a thin delegator.

---

## Directory structure

```
src/executor/
├── adapters/
│   ├── inbound/
│   │   └── executor.rs                     # Inbound adapter: implements ExecutorPort, delegates to ExecutorRunPort
│   └── outbound/
│       ├── file_system_writer.rs           # FileSystemWriter: only place that calls tokio::fs::write
│       ├── solidity_generator.rs           # Implements CodeGeneratorPort: assembles .sol files from BodiesJson
│       └── foundry_config_writer.rs        # Implements ConfigWriterPort: patches foundry.toml
├── ports/
│   ├── inbound/
│   │   └── executor_run_port.rs            # ExecutorRunPort: inbound contract between adapter and use case
│   └── outbound/
│       ├── code_generator_port.rs          # CodeGeneratorPort: generate(bodies, writer)
│       └── config_writer_port.rs           # ConfigWriterPort: write(config, writer)
└── use_cases/
    ├── execute.rs                          # ExecuteUseCase: owns outbound ports, implements ExecutorRunPort
    └── write_bodies.rs                     # write_bodies + write_config_json: persist BodiesJson and FuzzerConfigArtifact to disk
```

---

## Architecture layers

```
Orchestrator
    │
    └─ ExecutorPort (shared/ports)
           │
    Executor (adapters/inbound)                 ← implements ExecutorPort
           │
    ExecutorRunPort (ports/inbound)             ← inbound contract
           │
    ExecuteUseCase (use_cases)                  ← implements ExecutorRunPort, owns outbound ports
           │
    ├─ CodeGeneratorPort (ports/outbound)       ← outbound contract
    │      │
    │  SolidityGenerator (adapters/outbound)    ← implements CodeGeneratorPort
    │
    ├─ ConfigWriterPort (ports/outbound)        ← outbound contract
    │      │
    │  FoundryConfigWriter (adapters/outbound)  ← implements ConfigWriterPort
    │
    └─ FileSystemWriter (adapters/outbound)     ← raw I/O boundary, injected into outbound adapters
```

### Inbound adapter: `adapters/inbound/executor.rs`

Implements `ExecutorPort`. Holds `Box<dyn ExecutorRunPort>`. Delegates entirely to the use case: contains no logic of its own.

### Inbound port: `ports/inbound/executor_run_port.rs`

```rust
pub trait ExecutorRunPort: Send + Sync {
    async fn execute(&self, input: ExecutorInput) -> Result<()>;
}
```

### Use case: `use_cases/execute.rs`

`ExecuteUseCase` implements `ExecutorRunPort`. Owns all outbound dependencies:

```rust
pub struct ExecuteUseCase {
    writer: FileSystemWriter,
    generator: Arc<dyn CodeGeneratorPort>,
    config_writer: Arc<dyn ConfigWriterPort>,
}
```

Sequences four write operations: bodies JSON, config JSON, generated Solidity files, Foundry config patch. Before writing, it sets `bodies.meta.solidity` from `ExecutorInput.source_pragma` so the LLM never controls the pragma version. `SolidityGenerator` then re-reads the target source file (best-effort) to enforce the same pragma and to inject ABIEncoderV2 for Solidity 0.7.x.

### Outbound ports: `ports/outbound/`

| Trait | Purpose |
|---|---|
| `CodeGeneratorPort` | Assemble and write `.sol` files from `BodiesJson` |
| `ConfigWriterPort` | Patch `foundry.toml` with the new `FoundryConfig` |

### Outbound adapters: `adapters/outbound/`

`FileSystemWriter` is the single I/O boundary: the only struct allowed to call `tokio::fs`. Both `SolidityGenerator` and `FoundryConfigWriter` receive it as a parameter. It takes a `PathBuf` base path and enforces a path traversal guard on every write.

`SolidityGenerator` implements `CodeGeneratorPort`. It writes helper contracts (from `handler.helper_contracts`) above the Handler contract, injects `pragma experimental ABIEncoderV2;` for Solidity 0.7.x, and derives output paths from `bodies.meta.contract`: the LLM never decides where files go:

| File | Path |
|---|---|
| Handler | `test/fuzzming/{Contract}/{ContractName}Handler.sol` |
| Invariant test | `test/fuzzming/{Contract}/{ContractName}InvariantTest.sol` |

The `test/fuzzming/` namespace isolates generated files from the developer's own `test/` code.

`FoundryConfigWriter` implements `ConfigWriterPort`. Builds the `[profile.fuzzming]` and `[profile.fuzzming.invariant]` TOML sections from `FoundryConfig` fields. It always writes `test = "test/fuzzming"` directly under `[profile.fuzzming]` so that `forge test` and `forge build` are automatically scoped to the generated test directory whenever `FOUNDRY_PROFILE=fuzzming` is active: no CLI flags needed. All fuzzing parameters (`runs`, `depth`, `seed`, `max_test_rejects`, `dictionary_weight`) go under the `invariant` subsection where Foundry expects them. It reads the existing `foundry.toml` from disk, replaces only the fuzzming/coverage sections (including their sub-tables), and preserves everything else.

`SolidityGenerator` strips any trailing `{` from `constructorSignature` before appending its own opening brace, preventing double-brace errors when the LLM includes the brace in the signature string.

---

## File system layout (executor-owned paths)

```
{workspace_root}/
├── foundry.toml                          ← executor patches [profile.fuzzming]
├── test/
│   └── fuzzming/
│       └── {ContractName}/
│           ├── {ContractName}Handler.sol
│           └── {ContractName}InvariantTest.sol
└── .fuzzming/
    └── {ContractName}/
        ├── {ContractName}.bodies.json    ← LLM-generated test bodies; read back each round for Patch diffs
        └── {ContractName}.config.json   ← FuzzerConfigArtifact JSON; read back each round for Patch diffs
```

Both `.bodies.json` and `.config.json` go to `.fuzzming/` (not `test/fuzzming/`) because they are not Solidity: forge ignores them, and keeping them separate makes the directory intent clearer. They are also what the Reader loads on rounds 2+ so the LLM can return a `Patch` diff instead of re-generating everything.

---

## Data flow

```
Orchestrator
  │
  └─ Executor::execute(ExecutorInput)        ← ExecutorPort (inbound adapter)
       │
       └─ ExecuteUseCase::execute(input)     ← ExecutorRunPort (use case)
             │
             ├─ write_bodies(&bodies, &writer)
             │     serialises BodiesJson → .fuzzming/{Contract}/{Contract}.bodies.json
             │
             ├─ write_config_json(&fuzzer_config, contract, &writer)
             │     serialises FuzzerConfigArtifact → .fuzzming/{Contract}/{Contract}.config.json
             │
             ├─ generator.generate(&bodies, &writer)   ← CodeGeneratorPort
             │     assembles Handler .sol → test/fuzzming/{Contract}/{ContractName}.sol
             │     assembles InvariantTest .sol → test/fuzzming/{Contract}/{ContractName}.sol
             │
             └─ config_writer.write(&fuzzer_config, &writer)  ← ConfigWriterPort
                   patches [profile.fuzzming] in foundry.toml
```

---

## `FileSystemWriter`: path traversal guard

```rust
pub fn new(base_path: PathBuf) -> Self
pub async fn write_file(&self, path: &str, content: &str) -> Result<()>
```

Every write:
1. Creates the base directory if it does not exist (`create_dir_all`).
2. Canonicalises the base path.
3. Creates the target parent directory.
4. Canonicalises the target parent.
5. Asserts the target parent starts with the base: rejects paths that contain `..` or symlinks that escape `workspace_root`.

---

## Wiring at startup

```rust
let writer         = FileSystemWriter::new(workspace_root); // PathBuf
let generator      = Arc::new(SolidityGenerator);
let config_writer  = Arc::new(FoundryConfigWriter);
let use_case       = Box::new(ExecuteUseCase::new(writer, generator, config_writer));
let executor       = Executor::new(use_case);
```

`Executor` never imports `ExecuteUseCase`. All concrete types are resolved at the entry point only.

---

## Hard rules

- `Executor` reads only two developer-owned files: the target contract source (to preserve its pragma) and `foundry.toml` (to preserve non-fuzzming sections). All other reads and writes are confined to fuzzming-managed paths.
- `FileSystemWriter` is the only struct that calls `tokio::fs` for writing.
- The LLM never controls file paths: all paths are derived from `contract_name`.
