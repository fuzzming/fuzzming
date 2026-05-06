# Orchestrator Component

The Orchestrator is the **session controller** of FuzzMing. It drives the round loop for every target contract — reading context from disk, calling the LLM, writing generated tests, running forge, and deciding whether to continue or terminate. It delegates every unit of work to the other components; it owns no I/O and no business logic beyond loop control.

---

## Responsibility

One job: given a `SessionRequest`, run rounds until every contract reaches a terminal state, emit a report for each, and return the final `SessionOutcome`.

The session ends on **exhaustion or full coverage** — not on the first bug. When a bug is found, the orchestrator records it, removes the broken invariant from the next round's generated test, and continues hunting for more bugs in the remaining invariants.

---

## Directory structure

```
src/orchestrator/
├── adapters/
│   └── inbound/
│       └── orchestrator.rs          # Inbound adapter — implements OrchestratorPort, delegates to use case
├── ports/
│   └── inbound/
│       └── orchestrator_run_port.rs # OrchestratorRunPort — internal use-case contract
└── use_cases/
    ├── initialise_session.rs        # Builds SessionState from SessionRequest
    ├── run_round.rs                 # One round for one contract: LLM → strip confirmed bugs → Executor
    ├── run_session.rs               # Main loop: manages active contracts, accumulates bugs, terminates, reports
    └── check_termination.rs        # Pure decision: should this contract's session end?
```

---

## Architecture layers

```
Entry point (CLI / CiCD)
    │
    └─ OrchestratorPort (shared/ports)
           │
    Orchestrator (adapters/inbound)           ← implements OrchestratorPort
           │
    OrchestratorRunPort (ports/inbound)       ← internal contract
           │
    RunSessionUseCase (use_cases/run_session) ← owns the loop
           │
           ├─ initialise_session()
           ├─ run_round()   ──→ LlmEnginePort → ExecutorPort → FuzzerEnginePort
           ├─ check_termination()
           └─ ReporterPort
```

---

## Use cases

### `initialise_session`

Builds the initial `SessionState` from the incoming `SessionRequest`.

```rust
SessionState {
    rounds_remaining: request.max_rounds,
    current_round:    0,
    config:           request.config,
    found_bugs:       HashMap::new(),
}
```

### `run_round`

Runs the LLM and Executor for a single contract. The fuzzer is intentionally excluded — it is called once for all contracts after all `run_round` calls complete.

1. **LLM** — `llm_engine.run(signal)` → generates invariant bodies and foundry config
2. **Strip confirmed bugs** — for `Full` responses, removes already-confirmed invariant functions from `bodies.invariant_test.invariants` before passing to the executor (Option B deterministic removal)
3. **Executor** — writes generated files to disk (invariant test `.sol` + `foundry.toml` patch)

Returns `LlmSignal`.

**Signature:**

```rust
pub async fn run_round(
    signal: RoundSignal,
    llm_engine: &dyn LlmEnginePort,
    executor: &dyn ExecutorPort,
) -> Result<LlmSignal>
```

The `ExecutorInput` variant is determined by the LLM response:

| LLM response | `ExecutorInput` variant | When |
|---|---|---|
| `GenerationResponse::Full` | `ExecutorInput::Full` | Round 1 — write everything from scratch |
| `GenerationResponse::Patch` | `ExecutorInput::Patch` | Round N — apply diff to previous artifacts |

**Confirmed bug stripping** applies only to `Full` responses. For `Patch` responses the LLM is instructed via the prompt not to re-generate confirmed invariants.

### `check_termination`

Pure function. Maps a `FuzzReport` outcome to a `TerminationDecision`.

| `FuzzOutcome` | `rounds_remaining` | Decision |
|---|---|---|
| `Bug` | any | **continue** — accumulate and keep hunting |
| `FullCoverage` | any | terminate → `FullCoverage` |
| `DevTestFailed` | any | terminate → `DevTestFailed` |
| `Pass` | 0 | terminate → `Exhausted` |
| `Pass` | > 0 | continue |

`Bug` is no longer a terminal state. The session ends only on `Exhausted` or `FullCoverage`.

### `run_session` (main loop)

Owns the contract lifecycle. Each round has three parallel stages, followed by a single batched forge run, then per-contract bug accumulation and termination checks.

```
initialise_session(request) → SessionState { found_bugs: {} }
active = all target contract paths

loop:
    current_round += 1

    ── Stage 1: parallel reads ──────────────────────────────────────────
    try_join_all:
        for each contract → build_signal()
            tokio::try_join!(get_contract_context, get_fuzz_output,
                             get_coverage_context, get_existing_bodies,
                             get_existing_config)
            existing_foundry_config = FuzzerConfigArtifact::Foundry(fc) (None on round 1)
            confirmed_bugs = state.found_bugs[contract] (empty on round 1)

    ── Stage 2: parallel LLM + Executor ─────────────────────────────────
    try_join_all:
        for each contract → run_round(signal, llm, executor)
            llm_engine.run(signal) → LlmSignal
            strip confirmed invariants from Full response bodies
            executor.execute(input)          (writes .sol + foundry.toml)

    ── Stage 3: single forge run ─────────────────────────────────────────
    fuzzer_engine.run(all signals) → Vec<FuzzReport>

    rounds_remaining -= 1

    ── Bug accumulation + termination check (per contract) ───────────────
    for each (contract, report):
        if report.bugs not empty:
            state.found_bugs[contract].extend(report.bugs)   ← accumulate
        check_termination(report, state) → TerminationDecision
        if terminate:
            all_bugs = state.found_bugs[contract]            ← all rounds
            read ReportArtifacts from disk
            build call_sequences from all_bugs (not just last round)
            reporter.emit(SessionOutcome)
            remove contract from active

    if active is empty → break
```

The last round (`rounds_remaining` reaches 0) causes any still-passing contract to terminate with `Exhausted`. The final report carries every bug found across all rounds.

---

## Data model

### `SessionRequest` (input)

```rust
pub struct SessionRequest {
    pub target_paths: Vec<String>,   // e.g. ["src/Vault.sol", "src/Token.sol"]
    pub max_rounds:   u32,
    pub config:       SessionConfig,
    pub output_format: OutputFormat,
    pub ci_mode:      bool,
}
```

### `SessionState` (internal loop state)

```rust
pub struct SessionState {
    pub rounds_remaining: u32,
    pub current_round:    u32,
    pub config:           SessionConfig,
    /// All bugs found so far, keyed by contract name. Grows across rounds; never cleared.
    pub found_bugs:       HashMap<String, Vec<BugInfo>>,
}
```

### `RoundSignal` (per-contract per-round context)

Built fresh each round from disk. Carries everything the LLM and fuzzer need.

```rust
pub struct RoundSignal {
    pub round:                   u32,
    pub config:                  SessionConfig,
    pub contract_name:           String,                   // e.g. "Vault"
    pub contract_path:           String,                   // e.g. "src/Vault.sol"
    pub source_code:             String,
    pub fuzz_output:             Option<String>,           // None on round 1
    pub coverage_context:        Option<CoverageContext>,  // None on round 1
    pub existing_bodies:         Option<BodiesJson>,       // None on round 1
    pub existing_foundry_config: Option<FoundryConfig>,    // None on round 1; read from .fuzzming/{Contract}/{Contract}.config.json on round N
    pub confirmed_bugs:          Vec<BugInfo>,             // empty on round 1
}
```

`confirmed_bugs` is populated from `state.found_bugs[contract]` at the start of each round. It flows to both the LLM prompt (so the model avoids re-generating broken invariants) and `run_round` (so confirmed invariants are stripped from `Full` responses before the executor writes them).

### `SessionOutcome` (output)

```rust
pub struct SessionOutcome {
    pub reason:           TerminationReason,
    pub contract_name:    String,
    pub rounds_completed: u32,
    pub artifacts:        ReportArtifacts,
}
```

`ReportArtifacts.call_sequences` contains every bug found across all rounds, not just the termination round.

---

## Artifact paths

The orchestrator reads and writes artifacts under a `.fuzzming/` directory at the workspace root:

| Artifact | Path |
|---|---|
| LLM-generated bodies | `.fuzzming/{contract}/{contract}.bodies.json` |
| Fuzzer config (JSON) | `.fuzzming/{contract}/{contract}.config.json` |
| Forge fuzz output | `.fuzzming/{contract}/fuzz_output.txt` |
| LCOV coverage | `.fuzzming/{contract}/lcov.info` |

---

## Ports consumed

| Port | Direction | Used for |
|---|---|---|
| `LlmEnginePort` | outbound | Generate invariant test bodies |
| `ExecutorPort` | outbound | Write generated files to disk |
| `FuzzerEnginePort` | outbound | Run `forge test` and evaluate outcome |
| `ReaderPort` | outbound | Read source code and previous-round artifacts |
| `ReporterPort` | outbound | Emit formatted result when a contract terminates |

---

## Known limitations

- **LLM calls are concurrent, not CPU-parallel** — `try_join_all` interleaves LLM and Executor futures on the same async task. For true thread-level parallelism, futures would need to be spawned with `tokio::spawn`, which requires `'static` bounds on all port references.

- **Confirmed bug stripping only for Full responses** — `run_round` removes confirmed invariants from `GenerationResponse::Full` bodies before the executor writes them. For `Patch` responses, the LLM is relied on to follow the `CONFIRMED BUGS` prompt instruction and not re-add them.
