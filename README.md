# FuzzMing

FuzzMing is a language-agnostic, LLM-powered fuzzing assistant. It closes the loop between an LLM and a fuzzer — generating invariant test bodies, running them, reading the results, and iterating until it finds every bug, achieves full coverage, or exhausts the configured round budget.

The first supported stack is **Solidity + Foundry**. The architecture is intentionally built to absorb new languages and fuzzers as first-class targets without touching the core.

---

## Table of Contents

- [How it works](#how-it-works)
- [Continuous audit model](#continuous-audit-model)
- [Architecture vision: supporting any language, any fuzzer](#architecture-vision-supporting-any-language-any-fuzzer)
- [The shared data layer](#the-shared-data-layer)
- [Adding a new language or fuzzer](#adding-a-new-language-or-fuzzer)
- [Distribution strategy](#distribution-strategy)
- [Release strategy](#release-strategy)
- [Source layout](#source-layout)
- [Usage](#usage)
- [Development status](#development-status)

---

## How it works

Each round follows this sequence:

```
1. Reader reads the target source file(s) and any previous fuzz output / coverage gaps
2. Generator assembles a prompt and calls the configured LLM model
3. Executor writes generated test bodies → fuzzming-owned test files
4. Executor writes an adapted fuzzer config for the current round
5. FuzzerEngine runs the fuzzer subprocess
6. If any contract passes → FuzzerEngine runs the coverage tool
7. Orchestrator accumulates bugs, strips confirmed invariants, checks termination per contract
8. Reporter formats and emits the final result when a contract's session ends
```

The session ends on **exhaustion or full coverage**, not on the first bug. When a bug is found, the orchestrator records it, removes the broken invariant from the next round's test, tells the LLM what was already found, and continues hunting for more bugs in the remaining invariants.

---

## Continuous audit model

| Condition | Action |
|---|---|
| Bug confirmed (invariant falsified) | Record bug, strip that invariant, continue next round |
| Compilation error | Write compiler output as fuzz feedback, let LLM repair and retry next round |
| Developer test failed | Report immediately — fuzzing environment is broken |
| Full coverage reached | Report — no point continuing |
| Rounds exhausted | Report everything found across all rounds |

The final report always carries **every bug found across all rounds**, not just the last round's result. Exit code 1 is set whenever any bugs were found or developer tests failed — CI pipelines treat this as a build failure.

---

## Architecture vision: supporting any language, any fuzzer

FuzzMing is built on **hexagonal architecture with a sequential orchestration model**. The flow is linear — the Orchestrator calls each component in order and passes data between them. Components never call each other. All inter-component contracts are defined in `src/shared/ports/` so there is one place to find every boundary in the system.

```
SessionOrchestrator
    │
    ├── reader.get_*(RoundSignal)  ─────────────► context data
    │
    ├── generator.run(RoundSignal) ─────────────► LlmSignal
    │
    ├── executor.execute(ExecutorInput) ◄─────────┘
    │
    ├── fuzzer_engine.run(Vec<RoundSignal>) ──────► Vec<FuzzReport>
    │
    └── reporter.emit(SessionOutcome)
```

Every component is behind a port defined in `src/shared/ports/`. The Orchestrator only knows ports — never concrete types. Adding a new language or fuzzer means writing new adapters; the Orchestrator, the ports, and the shared data layer are untouched.

```
src/shared/ports/
    │
    ├── LlmEnginePort        ← Orchestrator → Generator
    ├── ExecutorPort         ← Orchestrator → Executor
    ├── FuzzerEnginePort     ← Orchestrator → FuzzerEngine
    ├── ReporterPort         ← Orchestrator → Reporter
    └── ReaderPort           ← Orchestrator → Reader
```

Internal extension points (language axis, fuzzer axis) stay in each component's own `ports/` folder — they are not inter-component communication.

---

## The shared data layer

`src/shared/` is the single source of truth for every data shape that crosses a component boundary.

```
src/shared/
├── ports/            Inter-component contracts — every boundary in one place
│
├── models/           All shared data structures — no direction, no I/O
│   ├── session_config.rs        model, llm_key, output_format, language, fuzzer, workspace_root
│   ├── session_state.rs         rounds_remaining, current_round, config, found_bugs (per contract)
│   ├── bodies_json.rs           LLM-generated test bodies (IndexMap preserves order)
│   ├── foundry_config.rs        Foundry fuzzing parameters
│   ├── coverage_context.rs      Uncovered lines/branches/functions with source snippets
│   ├── bug_info.rs              invariant_name + call_sequence
│   └── ...
│
├── requests/
│   ├── round_signal.rs     Per-round input: source, fuzz output, coverage, existing artifacts, confirmed_bugs
│   └── session_request.rs  Entry point → orchestrator: targets, max rounds, config
│
└── responses/
    ├── llm_signal.rs           LLM → Orchestrator: generated bodies + config
    ├── fuzz_report.rs          Fuzzer → Orchestrator: outcome, bugs, lcov path
    └── session_outcome.rs      Orchestrator → Reporter: termination reason + all artifacts
```

**Key properties:**

- Pure data — no methods that do I/O.
- Serializable — every artifact is `serde`-annotated for logging and replay.
- `RoundSignal.confirmed_bugs` — bugs found in previous rounds, carried to both the LLM prompt (so it avoids re-generating broken invariants) and `run_round` (which strips them from `Full` responses before the executor writes).

---

## Adding a new language or fuzzer

The full checklist to add, for example, **Rust + cargo-fuzz**:

1. **Reader adapter** — add `src/reader/adapters/rust_reader.rs` implementing `ContractReaderPort`.
2. **Executor language adapter** — add `src/executor/adapters/rust_generator.rs` implementing `CodeGeneratorPort`.
3. **Executor fuzzer adapter** — add `src/executor/adapters/cargo_fuzz_config_writer.rs` implementing `ConfigWriterPort`.
4. **Fuzzer adapter** — add `src/fuzzer/adapters/cargo_fuzz_runner.rs` implementing `TestRunnerPort`.
5. **Config artifact** — add `CargoFuzzConfig` to `src/shared/models/` and a `CargoFuzz` variant to `FuzzerConfigArtifact`.
6. **Generator prompt** — add a Rust-flavoured prompt template to `src/generator/use_cases/assemble_prompt.rs`.
7. **SessionConfig** — add `Language::Rust` and `Fuzzer::CargoFuzz` variants.
8. **Composition root** — add match arms in `CompositionRoot::build` for the new variants.

The orchestrator, reporter, session loop, and all shared ports require zero changes.

---

## Distribution strategy

FuzzMing is distributed as a **single native binary installed via package managers**:

| Channel | Command | Target audience |
|---|---|---|
| **Cargo** (crates.io) | `cargo install fuzzming` | Rust developers, CI pipelines |
| **Homebrew** | `brew install fuzzming` | macOS and Linux |
| **GitHub Releases** | Download `.tar.gz` / `.deb` / `.rpm` | Any platform, scripted installs |

There is no hosted service and no paid tier. The binary calls whichever LLM endpoint the user configures; FuzzMing itself is stateless.

---

## Release strategy

FuzzMing follows **semantic versioning** (`MAJOR.MINOR.PATCH`):

| Version bump | Trigger |
|---|---|
| `MAJOR` | Breaking change to `src/shared/` or the CLI surface |
| `MINOR` | New language or fuzzer support, new LLM adapter, new CLI flag |
| `PATCH` | Bug fix, prompt tuning, documentation |

---

## Source layout

```
src/
├── shared/       Shared contracts — models, ports, requests, responses
├── orchestrator/ Session loop, termination logic, round coordination
├── generator/    Prompt assembly, LLM calls, JSON parsing
├── fuzzer/       Forge subprocess, output parsing, coverage
├── reader/       Single read gateway — source files, fuzz output, lcov
├── executor/     Single write gateway — Solidity files, foundry.toml
├── reporter/     Stateless report formatter (terminal and CI output)
├── entry/        CLI (clap) and CI/CD (env vars) entry points
└── composition/  Composition root — the only file wiring concrete types
```

Each component follows the same hexagonal internal layout:

```
<component>/
├── adapters/
│   ├── inbound/    ← implements shared inter-component port, delegates to use case
│   └── outbound/   ← implements outbound ports, owns all I/O (fs, HTTP, subprocess)
├── ports/
│   ├── inbound/    ← contract between inbound adapter and use case
│   └── outbound/   ← contract between use case and outbound adapters
└── use_cases/      ← application logic, no I/O
```

**Hard rules:**

- No component calls another component directly.
- All inter-component port traits live in `src/shared/ports/`.
- Inbound adapters never hold outbound ports — they go through a use case.
- All OS calls (fs, process, HTTP) live in `adapters/outbound/` only.
- `CompositionRoot` is the only file that imports concrete types from multiple components.

---

## Usage

### Prerequisites

- Rust stable (2021 edition) — only needed if building from source
- [Foundry](https://getfoundry.sh) — `forge` must be on your `PATH`
- An API key for a supported LLM provider

### Install

```bash
# Build from source
cargo install --path .

# Via Cargo (once published)
cargo install fuzzming
```

### Run

```bash
fuzzming \
  --targets src/Vault.sol \
  --max-rounds 5 \
  --model openrouter/anthropic/claude-3.5-sonnet \
  --llm-key $OPENROUTER_KEY \
  --workspace-root /path/to/foundry-project
```

Sensitive values can be provided via environment variables to avoid them appearing in shell history:

```bash
export LLM_MODEL=groq/llama-3.3-70b-versatile
export LLM_KEY=$GROQ_KEY

fuzzming --targets src/Vault.sol --max-rounds 10
```

### Supported model providers

The `--model` prefix determines the provider. Pass the corresponding API key via `--llm-key`:

| Prefix | Provider | Example |
|---|---|---|
| `openrouter/` | OpenRouter | `openrouter/anthropic/claude-3.5-sonnet` |
| `groq/` | Groq | `groq/llama-3.3-70b-versatile` |
| `openai/` | OpenAI | `openai/gpt-4o` |
| `anthropic/` | Anthropic | `anthropic/claude-3-5-sonnet-20241022` |

### CI mode

Pass `--ci-mode` to post output as a GitHub PR comment instead of printing to stdout. The GitHub context is read from the environment automatically:

```bash
# In a GitHub Actions workflow
fuzzming \
  --targets src/Vault.sol \
  --max-rounds 5 \
  --model openrouter/anthropic/claude-3.5-sonnet \
  --llm-key $OPENROUTER_KEY \
  --ci-mode
# GITHUB_TOKEN, GITHUB_REPOSITORY, PR_NUMBER must be set in the environment
```

### Logging

FuzzMing emits structured logs via `tracing`. Set `RUST_LOG` to control verbosity:

```bash
# Round-by-round progress
RUST_LOG=info fuzzming --targets src/Vault.sol ...

# Everything including dependency internals
RUST_LOG=debug fuzzming --targets src/Vault.sol ...
```

Key `info`-level events: `session started`, `round started`, `LLM started/done`, `forge run started/finished`, `round complete — continuing`, `contract session terminated`.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Clean — all invariants passed, full coverage reached, or exhausted with no bugs found |
| `1` | Bugs found or developer tests failed — treat as build failure in CI |

---

## Development status

All components are implemented and wired. The full flow compiles and runs:

- Reader, Executor, Generator, Fuzzer, Reporter — fully implemented
- Orchestrator — session loop with continuous audit (accumulates bugs across rounds)
- Composition root — all concrete types wired
- CLI entry point — arg parsing, exit codes, tracing initialisation
- CI/CD adapter — `read_cicd_env()` is a stub pending implementation

### Cargo dependencies

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime |
| `async-trait` | Async methods on traits |
| `anyhow` | Error propagation |
| `serde` / `serde_json` | Artifact serialization |
| `reqwest` | HTTP calls (LLM endpoints, GitHub API) |
| `clap` | CLI argument parsing |
| `indexmap` | Insertion-order-preserving map for generated Solidity bodies |
| `litellm-rs` | LLM client — routes to any provider via model prefix |
| `tracing` / `tracing-subscriber` | Structured logging |
| `regex` | Comment stripping in `SolidityContractReader` |
