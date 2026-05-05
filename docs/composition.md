# Composition Root

`src/composition/composition_root.rs` is the **only file in the codebase that wires concrete types together**. Everything else depends only on ports (traits). This file is the answer to the question: *which adapter implements which port?*

---

## Responsibility

One job: given a `SessionConfig`, instantiate every concrete type in the right order, wire their dependencies, and return a `Box<dyn OrchestratorPort>` that the entry point can call.

The entry point calls it like this:

```rust
let orchestrator = CompositionRoot::build(config);
let outcome = orchestrator.run(request).await?;
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
│           └─ LiteLlmClient  (Box<dyn LlmClientPort>)
│
├─ FuzzerAdapter  (Box<dyn FuzzerEnginePort>)
│   └─ RunFuzzerUseCase  (Box<dyn FuzzerRunPort>)
│       ├─ ForgeRunner  (Box<dyn TestRunnerPort>)
│       └─ FileSystemFuzzerOutput  (Box<dyn FuzzerOutputPort>)
│
├─ Executor  (Box<dyn ExecutorPort>)
│   └─ ExecuteUseCase  (Box<dyn ExecutorRunPort>)
│       ├─ FileSystemWriter
│       ├─ SolidityGenerator  (Arc<dyn CodeGeneratorPort>)
│       └─ FoundryConfigWriter  (Arc<dyn ConfigWriterPort>)
│
├─ Reader  (Box<dyn ReaderPort>)
│   └─ ReadUseCase  (Box<dyn ReaderRunPort>)
│       ├─ SolidityContractReader  (Arc<dyn ContractReaderPort>)
│       ├─ FoundryCoverageReader  (Arc<dyn CoverageReaderPort>)
│       └─ FileSystemReader  (Arc — shared by both readers and the use case)
│
├─ Reporter  (Box<dyn ReporterPort>)
│   └─ TerminalOutput | PrCommentOutput  (Box<dyn OutputPort>)
│       selected by config.output_format
│
└─ Orchestrator  (Box<dyn OrchestratorPort>)
    └─ RunSessionUseCase  (Box<dyn OrchestratorRunPort>)
        receives all five components above as Box<dyn Port>
```

---

## Reporter output selection

The output adapter is the only runtime branch in the composition root:

```rust
let output: Box<dyn OutputPort> = match config.output_format {
    OutputFormat::Ci => {
        let token  = std::env::var("GITHUB_TOKEN").unwrap_or_default();
        let repo   = std::env::var("GITHUB_REPOSITORY").unwrap_or_default();
        let pr_num = std::env::var("PR_NUMBER").ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        Box::new(PrCommentOutput::new(token, repo, pr_num))
    }
    OutputFormat::Terminal => Box::new(TerminalOutput::new()),
};
```

CI mode reads GitHub Actions standard env vars. The CLI and CI/CD entry points do not need to know about them.

---

## Adding a new language or fuzzer

The composition root is where new stacks are activated. To add, for example, **Rust + cargo-fuzz**:

1. Implement the required adapters (see [README.md](../README.md#adding-a-new-language-or-fuzzer) checklist).
2. Add `Language::Rust` and `Fuzzer::CargoFuzz` variants to `SessionConfig`.
3. Add match arms in `CompositionRoot::build` to wire the new adapters when those variants are selected.

No other file in the codebase needs to change.

---

## Hard rule

`CompositionRoot` is the **only** file allowed to import concrete adapter types from multiple components. Any import of a concrete type outside this file is an architectural violation.
