# FuzzMing

FuzzMing is a language-agnostic, LLM-powered fuzzing assistant. It closes the loop between an LLM and a fuzzer — generating test bodies, running them, reading the results, and iterating until it finds a bug, achieves full coverage, or exhausts the configured round budget.

The first supported stack is **Solidity + Foundry**. The architecture is intentionally built to absorb new languages and fuzzers as first-class targets without touching the core.

---

## Table of Contents

- [How it works](#how-it-works)
- [Architecture vision: supporting any language, any fuzzer](#architecture-vision-supporting-any-language-any-fuzzer)
- [The shared data layer](#the-shared-data-layer)
- [Adding a new language or fuzzer](#adding-a-new-language-or-fuzzer)
- [Distribution strategy](#distribution-strategy)
- [Release strategy](#release-strategy)
- [Dependency management across language stacks](#dependency-management-across-language-stacks)
- [Source layout](#source-layout)
- [Usage](#usage)
- [Development status](#development-status)

---

## How it works

Each round follows this sequence:

```
1. Reader reads the target source file(s) and any previous fuzz output / coverage gaps
2. Generator assembles a prompt and calls the configured LLM endpoint
3. Executor writes generated test bodies → fuzzming-owned test files
4. Executor writes an adapted fuzzer config for the current round
5. FuzzerEngine runs the fuzzer subprocess
6. If the run exits clean → FuzzerEngine runs the coverage tool
7. Orchestrator evaluates the outcome and either stops or starts the next round
8. Reporter formats and emits the final result
```

Termination happens on the first of these conditions:

| Condition | What gets reported |
|---|---|
| Bug confirmed (invariant falsified) | Falsified invariant + call sequence + exact values |
| Developer test failed | Test name + assertion message |
| Full coverage reached | Coverage summary |
| Rounds exhausted | Uncovered lines with ±3 lines of source context |

---

## Architecture vision: supporting any language, any fuzzer

FuzzMing is built on **hexagonal architecture with a sequential orchestration model**. The flow is linear — the Orchestrator calls each component in order and passes data between them. Components never call each other. All inter-component contracts are defined in `src/shared/ports/` so there is one place to find every boundary in the system.

```
SessionOrchestrator
    │
    ├── generator.run(RoundSignal) ───────────────► LlmSignal
    │                                                    │
    ├── executor.execute(ExecutorInput) ◄────────────────┘
    │
    ├── fuzzer_engine.run(RoundSignal) ───────────► FuzzReport
    │
    └── reporter.emit(SessionOutcome)
```

Every component is behind a port defined in `src/shared/ports/`. The Orchestrator only knows ports — never concrete types. Adding a new language or fuzzer means writing new adapters; the Orchestrator, the ports, and the shared data layer are untouched.

```
src/shared/ports/        ← all inter-component contracts live here
    │
    ├── LlmEnginePort        ← Orchestrator → Generator
    ├── ExecutorPort         ← Orchestrator → Executor
    ├── FuzzerEnginePort     ← Orchestrator → FuzzerEngine
    ├── ReporterPort         ← Orchestrator → Reporter
    ├── ReaderPort           ← Orchestrator → Reader
    └── ReporterReaderPort   ← Reporter → Reader
```

Internal extension points (language axis, fuzzer axis) stay in each component's own `ports/` folder — they are not inter-component communication.

---

## The shared data layer

`src/shared/` is the single source of truth for every data shape that crosses a component boundary. It is the answer to the question: *"what does the core need to know?"*

```
src/shared/
├── ports/            Inter-component contracts — every boundary in one place
│   ├── llm_engine_port.rs       Orchestrator → Generator
│   ├── fuzzer_engine_port.rs    Orchestrator → FuzzerEngine
│   ├── executor_port.rs         Orchestrator → Executor
│   ├── reporter_port.rs         Orchestrator → Reporter
│   ├── reader_port.rs           Orchestrator → Reader
│   └── reporter_reader_port.rs  Reporter → Reader
│
├── models/           All shared data structures — no direction, no I/O
│   ├── bodies_json.rs           LLM-generated test bodies
│   ├── foundry_config.rs        Foundry fuzzing parameters
│   ├── executor_input.rs        BodiesJson + FuzzerConfigArtifact bundled for the executor
│   ├── fuzzer_config_artifact.rs  Enum wrapping per-fuzzer config (Foundry, Echidna, ...)
│   ├── contract_context.rs      Raw Solidity source code
│   ├── coverage_context.rs      Uncovered lines/branches/functions
│   ├── invariant_files.rs       File paths the system operates on
│   ├── session_config.rs        LLM key, language, fuzzer choice
│   ├── session_state.rs         Rounds remaining, current round
│   └── ...
│
├── requests/         Data flowing INTO components from the orchestrator
│   ├── round_signal.rs          Per-round input: source, fuzz output, coverage, existing artifacts
│   └── session_request.rs       Entry point → orchestrator: targets, max rounds, config
│
└── responses/        Data flowing OUT of components to the orchestrator
    ├── llm_signal.rs            LLM → Orchestrator: generated bodies + config
    ├── fuzz_report.rs           Fuzzer → Orchestrator: outcome + paths
    ├── termination_decision.rs  Use case → Orchestrator: stop or continue
    └── session_outcome.rs       Orchestrator → Reporter: final result
```

**Key properties of the shared data layer:**

- Pure data — no methods that do I/O or call external processes.
- Serializable — every artifact is `serde`-annotated so it can be logged, diffed, and replayed.
- Versioned — `bodies_json.rs` uses an explicit `schema_version` field so the LLM output contract can evolve without silent breakage.
- Language-neutral — `bodies_json.rs` describes test bodies as opaque strings paired with a `language` tag. The Solidity adapter knows what to do with `.sol`; a future Rust adapter would handle `.rs`. The core never inspects the string.

When a new language stack is added, the shared data layer may gain new context fields (e.g., a `RustCrateContext`) but existing fields are never removed or renamed across minor versions.

---

## Adding a new language or fuzzer

The full checklist to add, for example, **Rust + cargo-fuzz**:

1. **Reader adapter** — add `src/reader/adapters/rust_reader.rs` implementing `ContractReaderPort` to parse `.rs` source files.
2. **Executor language adapter** — add `src/executor/adapters/rust_generator.rs` implementing `CodeGeneratorPort` to write `fuzz_target!` harness files.
3. **Executor fuzzer adapter** — add `src/executor/adapters/cargo_fuzz_config_writer.rs` implementing `ConfigWriterPort` to write `Cargo.toml` fuzz config.
4. **Fuzzer adapter** — add `src/fuzzer/adapters/cargo_fuzz_runner.rs` implementing `TestRunnerPort` to run `cargo fuzz run`.
5. **Config artifact** — add `CargoFuzzConfig` to `src/shared/artifacts/` and a `CargoFuzz` variant to `FuzzerConfigArtifact`.
6. **Generator prompt** — add a Rust-flavoured prompt template to `src/generator/use_cases/assemble_prompt.rs`.
7. **SessionConfig** — add `Language::Rust` and `Fuzzer::CargoFuzz` variants.
8. **Composition root** — add match arms in `CompositionRoot::build` for the new variants.

The orchestrator, reporter, session loop, and `ExecutorPort` require zero changes.

---

## Distribution strategy

FuzzMing is distributed as a **single native binary installed via package managers**. This is the right model for a developer tool:

- Developers already live in the terminal. A CLI fits their workflow without asking them to visit a website or authenticate.
- A single self-contained binary has no runtime dependencies of its own — the only dependencies are the external toolchains the user already has (`forge`, `cargo`, etc.).
- Package managers provide versioning, reproducible upgrades, and rollback.

### Primary install channels

| Channel | Command | Target audience |
|---|---|---|
| **Cargo** (crates.io) | `cargo install fuzzming` | Rust developers, CI pipelines |
| **Homebrew** | `brew install fuzzming` | macOS and Linux |
| **GitHub Releases** | Download `.tar.gz` / `.deb` / `.rpm` | Any platform, scripted installs |
| **winget** | `winget install fuzzming` | Windows |

There is no hosted service and no paid tier in the distribution model. The binary calls whichever LLM endpoint the user configures; FuzzMing itself is stateless infrastructure.

### Why not a library?

A library (`cargo add fuzzming`) would make sense if the goal were to embed fuzzing assistance inside another tool. That is not the primary use case. The value is the autonomous loop: run the tool, get a result. A CLI expresses that contract directly. If third-party tooling needs to integrate FuzzMing programmatically, the `src/lib.rs` entry point is already there — it exposes the `SessionOrchestrator` and the port traits — but the library is a secondary interface, not the primary distribution artifact.

### Why not a web app?

A hosted web app would add authentication, request queuing, and server costs without adding value for the core workflow. The LLM calls are made directly from the user's machine to their chosen endpoint. This keeps sensitive code off third-party servers and gives teams with private LLM deployments a first-class experience.

---

## Release strategy

FuzzMing follows **semantic versioning** (`MAJOR.MINOR.PATCH`) with the following policy:

| Version bump | Trigger |
|---|---|
| `MAJOR` | Breaking change to the shared data layer (`src/shared/`) or to the CLI surface |
| `MINOR` | New language or fuzzer support, new LLM adapter, new CLI flag |
| `PATCH` | Bug fix, prompt tuning, documentation |

### Release artifacts per version

Each GitHub Release ships:

```
fuzzming-v{VERSION}-x86_64-unknown-linux-gnu.tar.gz
fuzzming-v{VERSION}-x86_64-apple-darwin.tar.gz
fuzzming-v{VERSION}-aarch64-apple-darwin.tar.gz
fuzzming-v{VERSION}-x86_64-pc-windows-msvc.zip
fuzzming_{VERSION}_amd64.deb
fuzzming-{VERSION}-1.x86_64.rpm
```

All targets are built by a single cross-compilation CI job (GitHub Actions + `cross`). The Homebrew formula and crates.io publish happen automatically on tag push via the same pipeline.

### Language support versioning

When a new language stack ships, it is gated behind a `--language <id>` flag. The flag accepts `solidity` (stable) and the new target (initially `experimental`). This allows the binary to ship multi-language support incrementally without breaking the default Solidity workflow. Experimental stacks graduate to stable in a `MINOR` release once their adapter suite passes the integration test matrix.

---

## Dependency management across language stacks

The binary itself has no runtime dependencies — it is statically linked (musl on Linux). All external toolchain dependencies are **runtime-detected**, not installed by FuzzMing.

### Runtime detection model

On startup, FuzzMing checks whether the required tools for the selected language stack are available on `PATH`:

| Language stack | Required on PATH |
|---|---|
| `--language solidity` | `forge` (Foundry) |
| `--language rust` (planned) | `cargo`, `cargo-fuzz` |
| `--language move` (planned) | `aptos` or `sui` CLI |

If a required tool is missing, FuzzMing exits with a clear error message and install instructions. It does **not** attempt to install toolchains on the user's behalf.

```
error: 'forge' not found on PATH.
  FuzzMing requires Foundry to run Solidity fuzzing.
  Install it with: curl -L https://foundry.paradigm.xyz | bash
```

### Compile-time feature flags

Language adapters are compiled into the binary unconditionally — there are no Cargo feature flags per language. This avoids the combinatorial release problem (N languages × M fuzzers = too many release variants) and means a single binary works for all stacks. The cost is a slightly larger binary; the benefit is that users get new stack support automatically on upgrade without reinstalling.

### Cargo dependencies

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime |
| `async-trait` | Async methods on traits |
| `anyhow` | Error propagation |
| `serde` / `serde_json` | Serialization of artifacts and session memory |
| `reqwest` | HTTP calls to LLM endpoints |
| `clap` | CLI argument parsing |

No language-specific crates are needed — the language adapters shell out to the external toolchain rather than calling language-specific Rust libraries. This keeps the dependency tree shallow and the binary portable.

---

## Source layout

```
src/
├── shared/              Shared contracts — models, ports, requests, responses
├── orchestrator/        Session loop + termination logic
├── generator/           Prompt assembly, test body generation, config adaptation
├── fuzzer/              Fuzzer subprocess execution and outcome evaluation
├── reader/              Single read gateway + all file and output parsers
├── executor/            Single write gateway — use_cases, ports, adapters
├── reporter/            Stateless report formatter (terminal and CI output)
├── entry/               CLI (clap) and CI/CD (env vars) entry points
└── composition/         Composition root — the only file wiring concrete types
```

Each component follows the same hexagonal internal layout:

```
<component>/
├── adapters/
│   ├── inbound/    ← receives external input, implements shared inter-component port, delegates to inbound port
│   └── outbound/   ← implements outbound port traits, owns all I/O (fs, HTTP, subprocess)
├── ports/
│   ├── inbound/    ← trait contract between inbound adapter and use case
│   └── outbound/   ← trait contract between use case and outbound adapters
└── use_cases/      ← application logic, no I/O, owns outbound port dependencies
```

**Hard rules:**

- No component calls another component directly — the Orchestrator sequences all calls.
- All inter-component port traits live in `src/shared/ports/`.
- Inbound adapters never hold outbound ports directly — they go through a use case via an inbound port.
- Internal extension points (language axis, fuzzer axis) live in the component's own `ports/outbound/`.
- `Reader` never writes. `Executor` never reads.
- `Executor` never touches developer-owned files.
- `CompositionRoot` is the only file that wires concrete types.
- All OS calls (fs, process, HTTP) live in `adapters/outbound/` only.

---

## Usage

### Prerequisites

- Rust stable (2021 edition) — only needed if building from source
- The toolchain for your target language stack (e.g., [Foundry](https://getfoundry.sh) for Solidity)
- An OpenAI-compatible LLM endpoint

### Install

```bash
# Via Cargo
cargo install fuzzming

# Via Homebrew (macOS / Linux)
brew install fuzzming

# From pre-built binary (example: Linux x86_64)
curl -L https://github.com/fuzzming/fuzzming/releases/latest/download/fuzzming-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv fuzzming /usr/local/bin/
```

### Run

```bash
fuzzming \
  --language solidity \
  --targets src/MyContract.sol \
  --max-rounds 10 \
  --llm-url https://api.openai.com/v1 \
  --llm-key sk-...
```

Environment variables are accepted for sensitive values:

```bash
export LLM_URL=https://api.openai.com/v1
export LLM_KEY=sk-...

fuzzming --language solidity --targets src/MyContract.sol --max-rounds 10
```

### CI/CD mode

Pass `--ci-mode` to emit structured output suitable for GitHub/GitLab PR comments.

```bash
fuzzming --language solidity --targets src/MyContract.sol --ci-mode
```

---

## Development status

The full architectural scaffolding is in place. Component implementations are stubs (`todo!()`). The structure, all trait contracts, and data types compile cleanly.

Implementation order that respects the dependency graph:

1. `Reader` + `FileSystemReader` — no external dependencies
2. `Executor` + `FileSystemWriter` — no external dependencies
3. `Generator` (LLM adapters, parsers, use cases)
4. `FuzzerEngine` (Foundry runner, outcome evaluation)
5. `Reporter` (format use cases, terminal and CI output)
6. `SessionOrchestrator` (wire everything via `CompositionRoot`)
7. First additional language stack (validates the port abstraction under real conditions)
