# Executor Component

The Executor is the **write gateway** of FuzzMing. After the Generator produces new bodies and config, the orchestrator packages them into an `ExecutorInput` and calls the Executor. The Executor writes the generated Solidity files and patches `foundry.toml`. It never reads files and never runs external processes.

---

## Responsibility

One job: take `BodiesJson` and `FuzzerConfigArtifact` from `ExecutorInput` and write them to disk. All logic for how to write is inside the use case — the inbound adapter is a thin delegator.

---

## Directory structure

```
src/executor/
├── adapters/
│   ├── inbound/
│   │   └── executor.rs                     # Inbound adapter — implements ExecutorPort, delegates to ExecutorRunPort
│   └── outbound/
│       ├── file_system_writer.rs           # FileSystemWriter — only place that calls tokio::fs::write
│       ├── solidity_generator.rs           # Implements CodeGeneratorPort — assembles .sol files from BodiesJson
│       └── foundry_config_writer.rs        # Implements ConfigWriterPort — patches foundry.toml
├── ports/
│   ├── inbound/
│   │   └── executor_run_port.rs            # ExecutorRunPort — inbound contract between adapter and use case
│   └── outbound/
│       ├── code_generator_port.rs          # CodeGeneratorPort — generate(bodies, writer)
│       └── config_writer_port.rs           # ConfigWriterPort — write(config, writer)
└── use_cases/
    ├── execute.rs                          # ExecuteUseCase — owns outbound ports, implements ExecutorRunPort
    └── write_bodies.rs                     # Pure function: serialise BodiesJson to disk
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

### Inbound adapter — `adapters/inbound/executor.rs`

Implements `ExecutorPort`. Holds `Box<dyn ExecutorRunPort>`. Delegates entirely to the use case — contains no logic of its own.

### Inbound port — `ports/inbound/executor_run_port.rs`

```rust
pub trait ExecutorRunPort: Send + Sync {
    async fn execute(&self, input: ExecutorInput) -> Result<()>;
}
```

### Use case — `use_cases/execute.rs`

`ExecuteUseCase` implements `ExecutorRunPort`. Owns all outbound dependencies:

```rust
pub struct ExecuteUseCase {
    writer: FileSystemWriter,
    generator: Arc<dyn CodeGeneratorPort>,
    config_writer: Arc<dyn ConfigWriterPort>,
}
```

Sequences the three write operations: bodies JSON, generated Solidity files, Foundry config patch.

### Outbound ports — `ports/outbound/`

| Trait | Purpose |
|---|---|
| `CodeGeneratorPort` | Assemble and write `.sol` files from `BodiesJson` |
| `ConfigWriterPort` | Patch `foundry.toml` with the new `FoundryConfig` |

### Outbound adapters — `adapters/outbound/`

`FileSystemWriter` is the single I/O boundary — the only struct allowed to call `tokio::fs`. Both `SolidityGenerator` and `FoundryConfigWriter` receive it as a parameter.

`SolidityGenerator` implements `CodeGeneratorPort`. It emits Solidity strings verbatim — it never interprets or reformats them.

`FoundryConfigWriter` implements `ConfigWriterPort`. Builds the `[profile.fuzzming]` TOML section from `FoundryConfig` fields, then patches `foundry.toml` using replace-or-append logic.

---

## Data flow

```
Orchestrator
  │
  └─ Executor::execute(ExecutorInput)        ← ExecutorPort (inbound adapter)
       │
       └─ ExecuteUseCase::execute(input)     ← ExecutorRunPort (use case)
             │
             ├─ write_bodies(&input.bodies, &writer)
             │     serialises BodiesJson → test/<Contract>.bodies.json
             │
             ├─ generator.generate(&input.bodies, &writer)   ← CodeGeneratorPort
             │     assembles Handler .sol + InvariantTest .sol
             │
             └─ config_writer.write(&input.fuzzer_config, &writer)  ← ConfigWriterPort
                   patches [profile.fuzzming] in foundry.toml
```

---

## Wiring at startup

```rust
let writer         = FileSystemWriter::new(base_path);
let generator      = Arc::new(SolidityGenerator);
let config_writer  = Arc::new(FoundryConfigWriter);
let use_case       = Box::new(ExecuteUseCase::new(writer, generator, config_writer));
let executor       = Executor::new(use_case);
```

`Executor` never imports `ExecuteUseCase`. All concrete types are resolved at the entry point only.

---

## Hard rules

- `Executor` never reads files — that is the Reader's job.
- `Executor` never touches developer-owned files — only fuzzming-managed paths.
- `FileSystemWriter` is the only struct that calls `tokio::fs`.
