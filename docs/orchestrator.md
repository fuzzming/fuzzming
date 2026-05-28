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
           ├─ [optional] SecurityAnalysisPort  ← pre-generation security analysis (patch rounds only)
           ├─ run_round()   ──→ LlmEnginePort → ExecutorPort
           ├─ FuzzerEnginePort
           ├─ check_termination()
           └─ ReporterPort
```

---

## Use cases

### `initialise_session`

Builds the initial `SessionState` from the incoming `SessionRequest`.

```rust
SessionState {
    rounds_remaining:    request.max_rounds,
    current_round:       0,
    config:              request.config,
    found_bugs:          HashMap::new(),
    full_coverage_streak: HashMap::new(),
    coverage_snapshots:  HashMap::new(),
    llm_failures:        HashMap::new(),
    security_analyses:   HashMap::new(),
}
```

### `run_round`

Runs the LLM and Executor for a single contract. The fuzzer is intentionally excluded — it is called once for all contracts after all `run_round` calls complete.

1. **LLM** — `llm_engine.run(signal)` → generates invariant bodies and foundry config, or returns `LlmStatus::Failed` (session does not crash)
2. **On LLM failure** — emits `StageStatus::Failed` for the LLM stage (closes the terminal spinner), prints the error via `emit_compile_error`, returns early; the error reason is recorded and injected into the next round
3. **Strip confirmed bugs** — for `Full` responses, removes already-confirmed invariant functions from `bodies.invariant_test.invariants` before passing to the executor
4. **Executor** — writes generated files to disk (invariant test `.sol` + `foundry.toml` patch)

Returns `LlmSignal`.

**Signature:**

```rust
pub async fn run_round(
    signal: RoundSignal,
    llm_engine: &dyn LlmEnginePort,
    executor: &dyn ExecutorPort,
    reporter: &dyn ReporterPort,
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
| `Bug` | 0 | terminate → `Exhausted` |
| `Bug` | > 0 | **continue**: accumulate and keep hunting |
| `CompileError` | 0 | terminate → `CompileError` |
| `CompileError` | > 0 | **continue**: let LLM repair and retry |
| `DevTestFailed` | 0 | terminate → `DevTestFailed` |
| `DevTestFailed` | > 0 | **continue**: let LLM repair and retry |
| `FullCoverage` | any | terminate → `FullCoverage` (checked separately via streak counter) |
| `Pass` | 0 | terminate → `Exhausted` |
| `Pass` | > 0 | continue |

`Bug`, `CompileError`, and `DevTestFailed` are not immediate terminal states — the session continues while rounds remain. Each terminates with its own `TerminationReason` when the round budget is exhausted.

### `run_session` (main loop)

Owns the contract lifecycle. Each round has four stages, followed by per-contract bug accumulation and termination checks.

```
initialise_session(request) → SessionState
active = all target contract paths

loop:
    current_round += 1

    ── Stage 1: parallel reads ──────────────────────────────────────────
    try_join_all:
        for each contract → build_signal()
            tokio::try_join!(get_contract_context, get_fuzz_output,
                             get_coverage_context, get_existing_bodies,
                             get_existing_config)
            source_pragma = extract_pragma_from_source(&source_code)  ← no extra I/O
            confirmed_bugs = state.found_bugs[contract] (empty on round 1)

    ── LLM failure injection ─────────────────────────────────────────────
    for each signal:
        if state.llm_failures[contract] exists:
            prepend "LLM PARSE FAILURE — ..." to signal.fuzz_output
            remove from state.llm_failures

    ── Stage 2: optional security analysis (patch rounds only) ──────────
    if security_analyzer is wired:
        filter signals: existing_bodies is Some AND fuzz_output has no
                        "COMPILATION ERROR" / "SETUP FAILURE" / "LLM PARSE FAILURE"
        join_all: sa.analyze(SecurityAnalysisRequest) per qualifying contract
        on success: state.security_analyses[contract] = analysis
                    signal.security_analysis = Some(analysis)
        on failure: warn and skip (session continues)

    ── Stage 3: parallel LLM + Executor ─────────────────────────────────
    try_join_all:
        for each contract → run_round(signal, llm, executor, reporter)
            llm_engine.run(signal) → LlmSignal
            on LLM failure: record in state.llm_failures, emit error, return early
            strip confirmed invariants from Full response bodies
            executor.execute(input)          (writes .sol + foundry.toml)

    ── Stage 4: single forge run ─────────────────────────────────────────
    fuzzer_engine.run(all signals) → Vec<FuzzReport>

    rounds_remaining -= 1

    ── Bug accumulation + termination check (per contract) ───────────────
    for each (contract, report):
        if report.bugs not empty:
            for each bug in report.bugs:              ← deduplicate by invariant name
                if bug.invariant_name not in found_bugs[contract]:
                    state.found_bugs[contract].push(bug)
        if report.outcome == Pass && lcov_path is Some:
            state.coverage_snapshots[contract].push(coverage_summary)
        decision = check_termination(report, state)
        if !decision.terminate:
            decision = check_full_coverage_streak(contract, report, state)
        if terminate:
            all_bugs = state.found_bugs[contract]            ← all rounds
            write .fuzzming/{Contract}/outcome.json
            reporter.emit(SessionOutcome {
                reason,
                contract_name,
                rounds_completed,
                bugs: all_bugs,
                coverage_snapshots: state.coverage_snapshots[contract],
                security_analysis: state.security_analyses[contract],
            })
            remove contract from active

    if active is empty → break
```

The last round (`rounds_remaining` reaches 0) causes any still-active contract to terminate. The final report carries every bug found across all rounds.

---

## Data model

### `SessionRequest` (input)

```rust
pub struct SessionRequest {
    pub target_paths: Vec<String>,   // e.g. ["src/Vault.sol", "src/Token.sol"]
    pub max_rounds:   u32,
    pub config:       SessionConfig,
}
```

### `SessionState` (internal loop state)

```rust
pub struct SessionState {
    pub rounds_remaining:     u32,
    pub current_round:        u32,
    pub config:               SessionConfig,
    /// All bugs found so far, keyed by contract name. Grows across rounds; never cleared.
    /// Deduplicated by invariant name — each unique invariant appears at most once.
    pub found_bugs:           HashMap<String, Vec<BugInfo>>,
    /// Consecutive full-coverage round count per contract.
    pub full_coverage_streak: HashMap<String, u32>,
    /// Per-round coverage summary strings for clean (bug-free) rounds.
    pub coverage_snapshots:   HashMap<String, Vec<String>>,
    /// LLM call error reasons from the previous round, keyed by contract name.
    /// Injected as fuzz_output prefix in the next round so the model can self-correct.
    pub llm_failures:         HashMap<String, String>,
    /// Latest security analysis per contract. Passed as previous_analysis into the next
    /// security analysis call so the LLM refines rather than restarts.
    pub security_analyses:    HashMap<String, String>,
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
    pub source_pragma:           String,                   // e.g. "=0.7.6" — extracted from source_code; never from LLM
    pub fuzz_output:             Option<String>,           // None on round 1
    pub coverage_context:        Option<CoverageContext>,  // None on round 1
    pub existing_bodies:         Option<BodiesJson>,       // None on round 1
    pub existing_foundry_config: Option<FoundryConfig>,    // None on round 1; read from .fuzzming/{Contract}/{Contract}.config.json on round N
    pub confirmed_bugs:          Vec<BugInfo>,             // empty on round 1
    pub security_analysis:       Option<String>,           // AI security analysis from before this round; None on round 1
}
```

`confirmed_bugs` is populated from `state.found_bugs[contract]` at the start of each round. It flows to both the LLM prompt (so the model avoids re-generating broken invariants) and `run_round` (so confirmed invariants are stripped from `Full` responses before the executor writes them).

`security_analysis` is injected by the orchestrator from `SecurityAnalysisPort` before calling the generator. Only set on patch rounds (round 2+) where the previous round was clean (no compile/setup/LLM error). Always `None` on round 1 — the generator's 3-stage analysis covers that.

### `SessionOutcome` (output)

```rust
pub struct SessionOutcome {
    pub reason:             TerminationReason,
    pub contract_name:      String,
    pub rounds_completed:   u32,
    pub bugs:               Vec<BugInfo>,        // all bugs found across all rounds
    pub coverage_snapshots: Vec<String>,         // per-round coverage summary strings
    pub security_analysis:  Option<String>,      // accumulated security analysis, if any
}
```

`bugs` carries every `BugInfo` accumulated across all rounds, deduplicated by invariant name — each unique invariant appears at most once regardless of how many rounds it fired. The `Exhausted` report uses `bugs` to show a count and list even when the session ran to completion without a definitive `Bug` termination.

`coverage_snapshots` accumulates one coverage summary string per round that produced a passing `forge coverage` result. These are forwarded to the reporter for display in the `FullCoverage` and `Exhausted` reports.

`security_analysis` is the last analysis produced by `SecurityAnalysisPort` for this contract. Printed after the outcome report by the CLI reporter.

`TerminationReason` values:

| Reason | Meaning |
|---|---|
| `Bug` | At least one invariant was falsified |
| `Exhausted` | Round budget used up; may or may not include bugs |
| `FullCoverage` | Full line/branch coverage sustained for `full_coverage_rounds` consecutive rounds |
| `DevTestFailed` | Developer tests failed (setUp revert, runtime panic) |
| `CompileError` | Generated code never compiled; rounds exhausted |

---

## Artifact paths

The orchestrator reads and writes artifacts under a `.fuzzming/` directory at the workspace root:

| Artifact | Path |
|---|---|
| LLM-generated bodies | `.fuzzming/{Contract}/{Contract}.bodies.json` |
| Fuzzer config (JSON) | `.fuzzming/{Contract}/{Contract}.config.json` |
| Forge fuzz output | `.fuzzming/{Contract}/fuzz_output.txt` |
| Coverage context (JSON) | `.fuzzming/{Contract}/coverage_context.json` |
| LCOV coverage (raw) | `.fuzzming/{Contract}/lcov.info` |
| Session outcome (JSON) | `.fuzzming/{Contract}/outcome.json` |

---

## Ports consumed

| Port | Direction | Used for |
|---|---|---|
| `LlmEnginePort` | outbound | Generate invariant test bodies |
| `ExecutorPort` | outbound | Write generated files to disk |
| `FuzzerEnginePort` | outbound | Run `forge test` and evaluate outcome |
| `ReaderPort` | outbound | Read source code and previous-round artifacts |
| `ReporterPort` | outbound | Emit formatted result when a contract terminates |
| `SecurityAnalysisPort` | outbound (optional) | Run a pre-generation security audit (patch rounds only) |

`SecurityAnalysisPort` is optional. When not wired (e.g. in tests), security analysis is simply skipped. The generator's round-1 3-stage analysis is unaffected either way.

---

## Known limitations

- **LLM calls are concurrent, not CPU-parallel** — `try_join_all` interleaves LLM and Executor futures on the same async task. For true thread-level parallelism, futures would need to be spawned with `tokio::spawn`, which requires `'static` bounds on all port references.

- **Confirmed bug stripping only for Full responses** — `run_round` removes confirmed invariants from `GenerationResponse::Full` bodies before the executor writes them. For `Patch` responses, the LLM is relied on to follow the `CONFIRMED BUGS` prompt instruction and not re-add them.

- **Security analysis skipped on error rounds** — when the previous round produced a compile, setup, or LLM failure, security analysis is intentionally skipped so the model can focus on fixing the error rather than processing new vulnerability suggestions.
