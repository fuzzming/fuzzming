# Composition Root

`src/composition/composition_root.rs` is the **only file in the codebase that wires concrete types together**. Everything else depends only on ports (traits). This file is the answer to the question: *which adapter implements which port?*

---

## Responsibility

One job: given a `SessionConfig`, instantiate every concrete type in the right order, wire their dependencies, and return a `Box<dyn OrchestratorPort>` that the entry point can call.

The entry point calls it like this:

```rust
let orchestrator = CompositionRoot::build(config);
let outcomes = orchestrator.run(request).await?;
```

Everything behind `orchestrator` is fully wired and ready to run.

---

## Why a single composition root?

- **Testability** — every component can be tested in isolation by providing mock implementations of its ports. The composition root is the one place that chooses the real adapters for production.
- **No circular imports** — each component's `adapters/` never imports another component. The composition root is the only file with cross-component imports.
- **Traceability** — the full wiring graph is visible in one file. If you want to know what implements `FuzzerEnginePort`, you read `composition_root.rs`.

---

## Full wiring graph

```
CompositionRoot::build(config)
│
├─ Generator  (Box<dyn LlmEnginePort>)
│   └─ GeneratorRunUseCase  (Box<dyn GeneratorRunPort>)
│       └─ LiteLlmGenerationAdapter  (Box<dyn GenerationPort>)
│           └─ LiteLlmClient  (Arc<dyn LlmClientPort>)
│               api key, config.max_tokens, config.llm_timeout_secs passed here
│           config.prompt_mode passed to adapter (selects Concise vs Guided system prompt)
│
├─ FuzzerAdapter  (Box<dyn FuzzerEnginePort>)
│   └─ RunFuzzerUseCase  (Box<dyn FuzzerRunPort>)
│       ├─ ForgeRunner  (Box<dyn TestRunnerPort>)
│       │     working_dir = config.workspace_root
│       ├─ FileSystemFuzzerOutput  (Box<dyn FuzzerOutputPort>)
│       │     workspace_root = config.workspace_root
│       └─ workspace_root: PathBuf  (used for compile-error isolation stash/restore)
│
├─ Executor  (Box<dyn ExecutorPort>)
│   └─ ExecuteUseCase  (Box<dyn ExecutorRunPort>)
│       ├─ FileSystemWriter  (base_path = config.workspace_root)
│       ├─ SolidityGenerator  (Arc<dyn CodeGeneratorPort>)
│       └─ FoundryConfigWriter  (Arc<dyn ConfigWriterPort>)
│
├─ Reader  (Box<dyn ReaderPort>)
│   └─ ReadUseCase  (Box<dyn ReaderRunPort>)
│       ├─ SolidityContractReader  (Arc<dyn ContractReaderPort>)
│       └─ FileSystemReader  (Arc — shared by both readers and the use case)
│             base_path = config.workspace_root
│
├─ Reporter  (Box<dyn ReporterPort>)
│   └─ TerminalOutput  (Box<dyn OutputPort>)
│
└─ Orchestrator  (Box<dyn OrchestratorPort>)
    └─ RunSessionUseCase  (Box<dyn OrchestratorRunPort>)
        receives all five components above as Box<dyn Port>
        optional SecurityAnalysisPort wired via with_security_analyzer()
```

---

## `LiteLlmClient` configuration

The LLM client receives the API key and two config values at construction time:

```rust
let llm_client: Arc<dyn LlmClientPort> = Arc::new(LiteLlmClient::new(
    &model,
    Some(api_key.as_str()),  // provider API key
    Some(0.1),               // temperature — fixed; not user-configurable
    config.max_tokens,       // Option<u32>; None = no output token limit
    config.llm_timeout_secs, // u64; default 120
));
```

`max_tokens` being `None` (or the value `0` from `fuzzming.config`) means the provider's default limit applies.

---

## `LiteLlmGenerationAdapter` — prompt mode

```rust
let generation_adapter = Box::new(LiteLlmGenerationAdapter::new(
    &model,
    &api_key,
    llm_client,
    prompt_mode,  // PromptMode::Concise or PromptMode::Guided
));
```

`PromptMode` is resolved at startup and passed through to the adapter. It controls how many design rules are included in the system prompt — not the JSON output schema, which is always the same.

---

## `LiteLlmSecurityAnalysisAdapter` — optional analyzer

The security analyzer shares the same `LiteLlmClient` as the generator. It is wired into the
orchestrator via `RunSessionUseCase::with_security_analyzer()` so patch rounds can request a
separate analysis pass before generation. The analyzer is optional; if it is not wired, the
session proceeds without the extra analysis stage.

---

## `RunFuzzerUseCase` — compile-error isolation

The fuzzer use case receives `workspace_root` as a third argument alongside the two outbound ports:

```rust
let fuzzer_use_case = Box::new(RunFuzzerUseCase::new(
    forge_runner,
    fuzzer_output,
    workspace.clone(), // needed for stash/restore of compile-erroring test dirs
));
```

When a compile error is detected the use case moves erroring `test/fuzzming/<Contract>/` directories to `.fuzzming-disabled/<Contract>/`, re-runs forge without them, assigns `CompileError` to those contracts, and restores all stashed directories before returning.

---

## Adding a new language or fuzzer

The composition root is where new stacks are activated. To add, for example, **Rust + cargo-fuzz**:

1. Implement the required adapters (see the checklist in [README.md](../README.md#contributing)):
   - `src/reader/adapters/rust_reader.rs` implementing `ContractReaderPort`
   - `src/executor/adapters/outbound/rust_generator.rs` implementing `CodeGeneratorPort`
   - `src/executor/adapters/outbound/cargo_fuzz_config_writer.rs` implementing `ConfigWriterPort`
   - `src/fuzzer/adapters/outbound/cargo_fuzz_runner.rs` implementing `TestRunnerPort`
2. Add `Language::Rust` and `Fuzzer::CargoFuzz` variants to `SessionConfig`.
3. Add `CargoFuzzConfig` to `src/shared/models/` and a `CargoFuzz` variant to `FuzzerConfigArtifact`.
4. Add a Rust-flavoured prompt template to the generator.
5. Add match arms in `CompositionRoot::build` to wire the new adapters when those variants are selected.

No other file in the codebase needs to change: orchestrator, reporter, session loop, and all shared ports are language/fuzzer-agnostic.

---

## Hard rule

`CompositionRoot` is the **only** file allowed to import concrete adapter types from multiple components. Any import of a concrete type from another component's `adapters/` directory outside this file is an architectural violation.
