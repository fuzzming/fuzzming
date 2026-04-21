# Shared Layer

`src/shared/` is the **shared contract layer** of FuzzMing. It contains every type and trait that crosses a component boundary. No component imports from another component — they only import from `src/shared/`.

This is the single source of truth for how the orchestrator, LLM, fuzzer, executor, reader, and reporter talk to each other.

---

## Directory structure

```
src/shared/
├── models/     — all shared data structures (no direction, no I/O)
├── ports/      — all traits (contracts each component must implement)
├── requests/   — data flowing INTO components from the orchestrator
└── responses/  — data flowing OUT of components to the orchestrator
```

---

## The four categories

### 1. Models — `models/`

Pure data structures. No direction — they are not inputs or outputs, they are just shapes that both sides agree on. Used inside requests, responses, and ports.

| Type | What it is |
|---|---|
| `BodiesJson` | The LLM's output — all the Solidity code for the Handler and invariant test contract |
| `HandlerBodies` | Handler contract fields: imports, state vars, ghost vars, constructor, functions (ordered map), target selectors |
| `InvariantTestBodies` | Invariant test contract: imports, state vars, setUp body, invariants (ordered map) |
| `BodiesMeta` | Metadata: contract name, path, Solidity version, timestamp |
| `FoundryConfig` | Foundry fuzzing parameters: runs, depth, seed, max_test_rejects, dictionary_weight, call_sequence_weights |
| `FuzzerConfigArtifact` | Enum wrapping fuzzer configs — currently `Foundry(FoundryConfig)`, designed for Echidna/Medusa later |
| `ExecutorInput` | What the executor receives: `BodiesJson` + `FuzzerConfigArtifact` |
| `AssembledPrompt` | LLM prompt built by `assemble_prompt` — list of `Message` (system + user) |
| `ContractContext` | Raw Solidity source code of the target contract |
| `CoverageContext` | List of `CoverageGap` — lines, branches, functions never executed |
| `CoverageGap` | A single uncovered location: file, line, type, surrounding source lines |
| `GapType` | Enum: `Line`, `Branch`, or `Function` |
| `InvariantFiles` | File paths the system operates on: invariant file, foundry.toml, lcov, fuzz output |
| `ReportArtifacts` | Data for the reporter: fuzz output, coverage summary, call sequences, round count |
| `SessionConfig` | LLM URL, API key, output format, CI mode, target language, fuzzer choice |
| `SessionState` | Rounds remaining, current round, session config |
| `OutputFormat` | Enum: `Terminal` or `Ci` |
| `Language` | Enum: `Solidity` (Rust, Vyper, Move reserved for future) |
| `Fuzzer` | Enum: `Foundry` (Echidna, Medusa, CargoFuzz reserved for future) |
| `InvariantSet` | A generated Solidity invariant file with its output path |
| `RunnerResult` | Raw process output: exit code, stdout, stderr |

**Why `IndexMap` for `functions` and `invariants`?**

`IndexMap<String, String>` instead of `HashMap` — insertion order is preserved so generated Solidity files are stable across runs. Same order every time, no unnecessary diffs.

**Why `Language` and `Fuzzer` enums?**

They control which concrete implementations the entry point selects at wiring time. Adding a new language or fuzzer = one enum variant + one new implementation. No changes to existing code.

---

### 2. Ports — `ports/`

Traits — the contracts each component must implement. The orchestrator depends only on ports, never on concrete component types.

| Trait | Implemented by | Signature |
|---|---|---|
| `LlmEnginePort` | `Generator` | `run(RoundSignal) -> LlmSignal` |
| `FuzzerEnginePort` | `Fuzzer` | `run(RoundSignal) -> FuzzReport` |
| `ExecutorPort` | `Executor` | `execute(ExecutorInput) -> ()` |
| `ReaderPort` | `Reader` | `get_contract_context`, `get_fuzz_output`, `get_coverage_context`, `get_invariant_files` |
| `ReporterPort` | `Reporter` | `emit(SessionOutcome)` |
| `ReporterReaderPort` | `Reader` | `get_report_artifacts` |

The orchestrator holds all components as `Box<dyn Port>`. It never knows which concrete type is behind the trait — resolved at the entry point.

---

### 3. Requests — `requests/`

Data flowing **into** components from the orchestrator. Carry everything the component needs — the component never reads files or calls other components.

| Type | Who receives it | What it carries |
|---|---|---|
| `RoundSignal` | LLM, Fuzzer | Round number, session config, contract source, fuzz output, coverage gaps, existing bodies, existing foundry config |
| `SessionRequest` | Orchestrator | Target contract paths, max rounds, session config, output format, CI mode |

`RoundSignal` is the central type. Every round the orchestrator assembles it from all current context and passes it to the LLM and the Fuzzer. Neither component stores state between rounds — everything they need is in the signal.

---

### 4. Responses — `responses/`

Data flowing **out** of components to the orchestrator after completing work.

| Type | Who produces it | What it carries |
|---|---|---|
| `LlmSignal` | LLM | Status (Done/Failed), generated `BodiesJson`, generated `FoundryConfig`, optional failure reason |
| `FuzzReport` | Fuzzer | Outcome (Bug/Pass/FullCoverage/DevTestFailed), paths to fuzz output and lcov |
| `TerminationDecision` | Orchestrator use case | Whether to stop, why, which paths to report |
| `SessionOutcome` | Orchestrator | Final result for the Reporter: termination reason + artifact paths |

**`FuzzOutcome` values:**

| Value | Meaning |
|---|---|
| `Bug` | Invariant broken — vulnerability found |
| `Pass` | All invariants held |
| `FullCoverage` | All lines/branches covered |
| `DevTestFailed` | Developer's own tests failed before fuzzing |

---

## How the types connect

```
Entry point
  │
  └─ SessionRequest (requests/)
       │
       └─ Orchestrator
             │
             ├─ Reader (ReaderPort)
             │     returns ContractContext, CoverageContext, InvariantFiles  ← models/
             │
             ├─ assembles RoundSignal  ← requests/
             │
             ├─ Llm.run(RoundSignal)
             │     returns LlmSignal { BodiesJson, FoundryConfig }           ← responses/ + models/
             │
             ├─ Executor.execute(ExecutorInput { BodiesJson, FuzzerConfigArtifact })  ← models/
             │
             ├─ Fuzzer.run(RoundSignal)
             │     returns FuzzReport { FuzzOutcome, FuzzPaths }             ← responses/
             │
             ├─ check termination → TerminationDecision                      ← responses/
             │
             └─ Reporter.emit(SessionOutcome)                                 ← responses/
```

Every arrow crosses through a type defined in `src/shared/`. No component has any other import path to another component.

---

## Design rules

**Components never call each other.** The orchestrator is the only coordinator. If the LLM needed data from the Reader, the orchestrator reads it and passes it in the signal.

**Ports defined by the consumer.** `LlmEnginePort` lives in `shared/ports/` because the orchestrator consumes it — not because the LLM provides it.

**Requests carry everything.** `RoundSignal` is fat by design — it carries all context the component might need. No hidden state, no side reads.

**Responses are minimal.** Components return only what the orchestrator needs next. `LlmSignal` carries `BodiesJson` and `FoundryConfig` — nothing else.

**Models have no direction.** A model is not a request or a response — it is a shape. `BodiesJson` appears in both `LlmSignal` (response) and `ExecutorInput` (used in a port call). It lives in `models/` because it belongs to neither direction.
