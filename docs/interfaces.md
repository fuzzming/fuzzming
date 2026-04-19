# Interfaces

`src/interfaces/` is the **shared contract layer** of FuzzMing. It contains every type and trait that crosses a component boundary. No component imports from another component — they only import from `interfaces/`.

This is the single source of truth for how the orchestrator, LLM, fuzzer, executor, reader, and reporter talk to each other.

---

## Directory structure

```
src/interfaces/
├── artifacts/      — data produced and consumed by components (generated Solidity, config, results)
├── contexts/       — read-only snapshots the orchestrator passes to components
├── requests/       — inputs sent from the orchestrator to components
├── responses/      — outputs returned from components to the orchestrator
├── state/          — session-level configuration and runtime state
└── ports/          — traits (contracts) each component must implement
```

---

## The five categories

### 1. Artifacts — `artifacts/`

Artifacts are **produced data** — things that get generated, written to disk, or passed between components. They are the payload that flows through the system.

| Type | What it is |
|---|---|
| `BodiesJson` | The LLM's output — all the Solidity code for the Handler and invariant test contract, structured as a JSON object |
| `HandlerBodies` | The Handler contract broken into fields: imports, state vars, ghost vars, constructor, functions (ordered map), target selectors |
| `InvariantTestBodies` | The invariant test contract: imports, state vars, setUp body, invariants (ordered map) |
| `BodiesMeta` | Metadata about the generation: contract name, path, Solidity version, timestamp |
| `FoundryConfig` | Foundry fuzzing parameters: runs, depth, seed, max_test_rejects, dictionary_weight, call_sequence_weights, current foundry.toml content |
| `FuzzerConfigArtifact` | Enum wrapper around fuzzer configs — currently only `Foundry(FoundryConfig)`, designed to add Echidna/Medusa later |
| `ExecutorInput` | What the executor receives: `BodiesJson` + `FuzzerConfigArtifact` bundled together |
| `AssembledPrompt` | The LLM prompt built by `assemble_prompt` use case — a list of `Message` (system + user) with round number and context sections |
| `InvariantSet` | A generated Solidity invariant file with its output path |
| `RunnerResult` | Raw output from running a process: exit code, stdout, stderr |

**Why `IndexMap` in `HandlerBodies` and `InvariantTestBodies`?**

`functions` and `invariants` are `IndexMap<String, String>` instead of `HashMap`. Insertion order is preserved so the generated Solidity files are stable across runs — same order every time, no unnecessary diffs.

---

### 2. Contexts — `contexts/`

Contexts are **read-only snapshots** the orchestrator collects from the Reader and passes to components via `RoundSignal`. Components never read files themselves — they receive context.

| Type | What it contains |
|---|---|
| `ContractContext` | The raw Solidity source code of the target contract as a string |
| `CoverageContext` | List of `CoverageGap` — lines, branches, or functions never executed during fuzzing |
| `CoverageGap` | A single uncovered location: file path, line number, gap type, and surrounding source lines for context |
| `GapType` | Enum: `Line`, `Branch`, or `Function` |
| `InvariantFiles` | File paths the system needs to operate: invariant test file, foundry.toml, lcov report, fuzz output log |
| `ReportArtifacts` | Data collected for the reporter: fuzz output string, coverage summary, call sequences, round count |

---

### 3. Requests — `requests/`

Requests are **inputs** the orchestrator sends to components. They carry everything the component needs to do its job — the component never reaches out to read anything itself.

| Type | Who receives it | What it carries |
|---|---|---|
| `RoundSignal` | LLM, Fuzzer | Round number, session config, contract source, fuzz output, coverage gaps, existing bodies, existing foundry config |
| `SessionRequest` | Orchestrator (entry point → orchestrator) | Target contract paths, max rounds, session config, output format, CI mode |

**`RoundSignal` is the central request type.** Every round the orchestrator assembles it from all current context and passes it to both the LLM and the Fuzzer. Neither component stores state between rounds — everything they need is in the signal.

---

### 4. Responses — `responses/`

Responses are **outputs** components return to the orchestrator after completing their work.

| Type | Who produces it | What it carries |
|---|---|---|
| `LlmSignal` | LLM component | Status (Done/Failed), generated `BodiesJson`, generated `FoundryConfig`, optional failure reason |
| `FuzzReport` | Fuzzer component | Outcome (Bug/Pass/FullCoverage/DevTestFailed), paths to fuzz output and lcov files |
| `TerminationDecision` | Orchestrator use case | Whether to stop the session, why, and which paths to report |
| `SessionOutcome` | Orchestrator | Final result passed to the Reporter: termination reason + artifact paths |

**`FuzzOutcome` enum values:**

| Value | Meaning |
|---|---|
| `Bug` | An invariant was broken — a vulnerability was found |
| `Pass` | All invariants held for this round |
| `FullCoverage` | All lines/branches covered — no more gaps to explore |
| `DevTestFailed` | Developer's own tests failed before fuzzing started |

---

### 5. State — `state/`

State holds **session-level configuration and runtime tracking** — values that are set once at startup and carried through the entire session.

| Type | What it contains |
|---|---|
| `SessionConfig` | LLM URL, LLM API key, output format, CI mode flag, target language, fuzzer choice |
| `SessionState` | Rounds remaining, current round number, session config |
| `OutputFormat` | Enum: `Terminal` or `Ci` |
| `Language` | Enum: `Solidity` (Rust, Vyper, Move reserved for future) |
| `Fuzzer` | Enum: `Foundry` (Echidna, Medusa, CargoFuzz reserved for future) |

`Language` and `Fuzzer` enums control which concrete implementations the entry point selects at wiring time. Adding a new language or fuzzer means adding an enum variant and a new implementation — no changes to existing code.

---

### 6. Ports — `ports/`

Ports are **traits** — the contracts each component must implement. The orchestrator depends only on ports, never on concrete component types. This is what makes components swappable.

| Trait | Implemented by | Signature |
|---|---|---|
| `LlmEnginePort` | `Llm` | `run(RoundSignal) -> LlmSignal` |
| `FuzzerEnginePort` | `Fuzzer` | `run(RoundSignal) -> FuzzReport` |
| `ExecutorPort` | `Executor` | `execute(ExecutorInput) -> ()` |
| `ReaderPort` | `Reader` | `get_contract_context`, `get_fuzz_output`, `get_coverage_context`, `get_invariant_files` |
| `ReporterPort` | `Reporter` | `emit(SessionOutcome)` |

The orchestrator holds all components as `Box<dyn Port>`. It never knows which concrete type is behind the trait — that is resolved at the entry point.

---

## How the types connect

```
Entry point
  │
  └─ SessionRequest
       │
       └─ Orchestrator
             │
             ├─ reads from Reader (ReaderPort)
             │     ContractContext, CoverageContext, InvariantFiles, Option<String> fuzz output
             │
             ├─ assembles RoundSignal  ← puts everything above into one struct
             │
             ├─ LLM (LlmEnginePort).run(RoundSignal)
             │     returns LlmSignal { BodiesJson, FoundryConfig }
             │
             ├─ Executor (ExecutorPort).execute(ExecutorInput { BodiesJson, FuzzerConfigArtifact })
             │
             ├─ Fuzzer (FuzzerEnginePort).run(RoundSignal)
             │     returns FuzzReport { FuzzOutcome, FuzzPaths }
             │
             ├─ check termination → TerminationDecision
             │
             └─ Reporter (ReporterPort).emit(SessionOutcome)
```

Every arrow in this diagram crosses through a type defined in `interfaces/`. No component has any other import path to another component.

---

## Design rules

**Components never call each other.** The orchestrator is the only coordinator. If the LLM needed data from the Reader, the orchestrator reads it and passes it in the signal — the LLM never calls the Reader directly.

**Ports define the direction.** A port is always defined by the consumer, not the provider. `LlmEnginePort` lives in `interfaces/ports/` because the orchestrator consumes it. The LLM component implements it.

**Requests carry everything.** `RoundSignal` is fat by design — it carries all context the component might need. Components have no reason to store state between rounds or reach outside their boundary.

**Responses are minimal.** Components return only what the orchestrator needs next. `LlmSignal` carries `BodiesJson` and `FoundryConfig` — nothing else. The orchestrator decides what to do with them.
