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

Pure data structures. No direction — they are not inputs or outputs, they are just shapes that both sides agree on.

| Type | What it is |
|---|---|
| `BodiesJson` | The LLM's output — all the Solidity code for the Handler and invariant test contract |
| `HandlerBodies` | Handler contract fields: imports, state vars, ghost vars, constructor, functions (ordered map), target selectors. No `output_path` — paths are derived by the executor. |
| `InvariantTestBodies` | Invariant test contract: imports, state vars, setUp body, invariants (ordered map). No `output_path`. |
| `BodiesMeta` | Metadata: contract name, contract path, Solidity version, timestamp |
| `FoundryConfig` | Foundry fuzzing parameters: runs, depth, seed, max_test_rejects, dictionary_weight, call_sequence_weights |
| `FuzzerConfigArtifact` | Enum wrapping fuzzer configs — currently `Foundry(FoundryConfig)` |
| `ExecutorInput` | What the executor receives: `BodiesJson` + `FuzzerConfigArtifact` |
| `AssembledPrompt` | LLM prompt built by `assemble_prompt` — list of `Message` (system + user) |
| `ContractContext` | Raw Solidity source code of the target contract (comments stripped) |
| `CoverageContext` | List of `CoverageGap` + summary counts (line_found/hit, branch_found/hit, function_found/hit) |
| `CoverageGap` | A single uncovered location: file, line, type, surrounding source lines |
| `GapType` | Enum: `Line`, `Branch`, or `Function` |
| `ReportArtifacts` | Data for the reporter: fuzz output string, coverage summary, call sequences, round count |
| `SessionConfig` | LLM URL, API key, output format, CI mode, target language, fuzzer choice, `workspace_root: PathBuf` |
| `SessionState` | Rounds remaining, current round, session config, all bugs found so far (keyed by contract name) |
| `OutputFormat` | Enum: `Terminal` or `Ci` |
| `Language` | Enum: `Solidity` (Rust, Vyper, Move reserved for future) |
| `Fuzzer` | Enum: `Foundry` (Echidna, Medusa, CargoFuzz reserved for future) |
| `RunnerResult` | Raw process output: exit code, stdout, stderr |

**`workspace_root: PathBuf`**

`SessionConfig.workspace_root` is a `PathBuf`, not a `String`. All path construction uses `PathBuf::join` — no ad-hoc string concatenation.

**Why `IndexMap` for `functions` and `invariants`?**

`IndexMap<String, String>` instead of `HashMap` — insertion order is preserved so generated Solidity files are stable across runs.

**Why no `output_path` in `HandlerBodies` / `InvariantTestBodies`?**

Previously the LLM was expected to fill in `output_path`. This was removed because: (1) the LLM could write any path including paths that escape the workspace, and (2) the executor derives paths deterministically from `contract_name` — the LLM has no role in deciding them.

---

### 2. Ports — `ports/`

Traits — the contracts each component must implement. The orchestrator depends only on ports, never on concrete component types.

| Trait | Implemented by | Signature |
|---|---|---|
| `LlmEnginePort` | `Generator` | `run(RoundSignal) -> Result<LlmSignal>` |
| `FuzzerEnginePort` | `Fuzzer` | `run(Vec<RoundSignal>) -> Result<Vec<FuzzReport>>` |
| `ExecutorPort` | `Executor` | `execute(ExecutorInput) -> Result<()>` |
| `ReaderPort` | `Reader` | `get_contract_context`, `get_coverage_context` |
| `ReporterPort` | `Reporter` | `emit(SessionOutcome)` |
| `ReporterReaderPort` | `Reporter`'s reader | `get_report_artifacts() -> Result<ReportArtifacts>` |

**`FuzzerEnginePort` is batch:** one call covers all contracts in the round. Input and output are parallel `Vec`s in the same order.

The orchestrator holds all components as `Box<dyn Port>`. It never knows which concrete type is behind the trait — resolved at the entry point.

---

### 3. Requests — `requests/`

Data flowing **into** components from the orchestrator. Carry everything the component needs — the component never reads files or calls other components.

| Type | Who receives it | What it carries |
|---|---|---|
| `RoundSignal` | LLM, Fuzzer | Round number, session config, `contract_name`, `contract_path`, source code, fuzz output, coverage gaps, existing bodies, existing foundry config |
| `SessionRequest` | Orchestrator | Target contract paths, max rounds, session config |

**`RoundSignal` fields:**

```rust
pub struct RoundSignal {
    pub round: u32,
    pub config: SessionConfig,
    pub contract_name: String,             // e.g. "Vault" — stem of the target file
    pub contract_path: String,             // e.g. "src/Vault.sol" — workspace-relative
    pub source_code: String,
    pub fuzz_output: Option<String>,
    pub coverage_context: Option<CoverageContext>,
    pub existing_bodies: Option<BodiesJson>,
    pub existing_foundry_config: Option<FoundryConfig>,
    pub confirmed_bugs: Vec<BugInfo>,      // bugs found in previous rounds — empty on round 1
}
```

`contract_name` and `contract_path` are set by the orchestrator from the CLI `--targets` argument — never from the LLM.

`confirmed_bugs` is populated from `SessionState.found_bugs[contract]` at the start of each round. It has two roles:
1. **LLM prompt** — the generator renders a `CONFIRMED BUGS` section so the model avoids re-generating already-broken invariants.
2. **Executor stripping** — `run_round` removes confirmed invariant names from a `Full` LLM response before the executor writes the file, so broken invariants are never included in the next forge run.

---

### 4. Responses — `responses/`

Data flowing **out** of components to the orchestrator after completing work.

| Type | Who produces it | What it carries |
|---|---|---|
| `LlmSignal` | LLM | Status (Done/Failed), generated `BodiesJson`, generated `FoundryConfig` |
| `FuzzReport` | Fuzzer | `outcome: FuzzOutcome`, `lcov_path: Option<PathBuf>` |
| `TerminationDecision` | Orchestrator use case | Whether to stop, why |
| `SessionOutcome` | Orchestrator | Final result for the Reporter: termination reason |

**`FuzzReport`:**

```rust
pub struct FuzzReport {
    pub outcome: FuzzOutcome,
    pub lcov_path: Option<PathBuf>,  // absolute path to .fuzzming/{Contract}/lcov.info
}
```

`lcov_path` is `Some` only when `outcome` is `Pass` and coverage ran successfully. The orchestrator passes it to the reader for the next round's `CoverageContext`.

**`FuzzOutcome` values:**

| Value | Meaning | Terminal? |
|---|---|---|
| `Bug` | Invariant broken — vulnerability found | **No** — accumulate bug, strip invariant, continue |
| `Pass` | All invariants held | Only when `rounds_remaining == 0` (`Exhausted`) |
| `FullCoverage` | All lines/branches covered | Yes |
| `DevTestFailed` | Developer's own tests failed before fuzzing | Yes |

`Bug` is not a terminal state. The session continues until `Exhausted` or `FullCoverage`. The final report carries every bug found across all rounds.

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
             │     get_contract_context(path) → ContractContext       ← models/
             │     get_coverage_context(lcov) → Option<CoverageContext>
             │
             ├─ assembles Vec<RoundSignal>  ← requests/
             │     (one per contract, carries contract_name + contract_path)
             │
             ├─ Llm.run(RoundSignal)  (parallel, one per contract)
             │     returns LlmSignal { BodiesJson, FoundryConfig }    ← responses/ + models/
             │
             ├─ Executor.execute(ExecutorInput)  (parallel, one per contract)
             │     writes test/fuzzming/{Contract}/ and .fuzzming/{Contract}/
             │
             ├─ Fuzzer.run(Vec<RoundSignal>)  (one call, all contracts)
             │     returns Vec<FuzzReport> { FuzzOutcome, lcov_path } ← responses/
             │
             ├─ check_termination per contract → TerminationDecision  ← responses/
             │
             └─ Reporter.emit(SessionOutcome)                          ← responses/
```

Every arrow crosses through a type defined in `src/shared/`. No component has any other import path to another component.

---

## Design rules

**Components never call each other.** The orchestrator is the only coordinator.

**Ports defined by the consumer.** `LlmEnginePort` lives in `shared/ports/` because the orchestrator consumes it — not because the LLM provides it.

**Requests carry everything.** `RoundSignal` is fat by design — no hidden state, no side reads.

**Responses are minimal.** Components return only what the orchestrator needs next.

**Models have no direction.** A model is not a request or a response — it is a shape. `BodiesJson` appears in both `LlmSignal` (response) and `ExecutorInput` (port call). It lives in `models/` because it belongs to neither direction.

**The LLM never controls paths.** `BodiesJson` has no `output_path` fields. All paths are derived by the executor from `contract_name`.
