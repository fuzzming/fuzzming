# Fuzzer Component

The Fuzzer is the **execution gateway** of FuzzMing. It runs `forge test` against the generated invariant tests, evaluates the result, and — if tests pass — runs `forge coverage` to generate an `lcov.info` file. It never writes Solidity files and never calls the LLM.

---

## Responsibility

One job: given a `RoundSignal`, run forge and return a `FuzzReport` with the outcome. All subprocess logic lives in the outbound adapter — the use case only orchestrates and evaluates.

---

## Directory structure

```
src/fuzzer/
├── adapters/
│   ├── inbound/
│   │   └── fuzzer.rs                   # Inbound adapter — implements FuzzerEnginePort, delegates to FuzzerRunPort
│   └── outbound/
│       └── forge_runner.rs             # ForgeRunner — only place that spawns forge subprocesses
├── ports/
│   ├── inbound/
│   │   └── fuzzer_run_port.rs          # FuzzerRunPort — inbound contract between adapter and use case
│   └── outbound/
│       └── test_runner_port.rs         # TestRunnerPort — outbound contract for running forge
└── use_cases/
    ├── run_fuzzer_session.rs           # RunFuzzerUseCase — implements FuzzerRunPort, owns TestRunnerPort
    ├── evaluate_outcome.rs             # Pure function: RunnerResult → FuzzOutcome
    ├── run_fuzzer.rs                   # Thin wrapper: calls runner.run_test()
    └── run_coverage.rs                 # Thin wrapper: calls runner.run_coverage()
```

---

## Architecture layers

```
Orchestrator
    │
    └─ FuzzerEnginePort (shared/ports)
           │
    Fuzzer (adapters/inbound)               ← implements FuzzerEnginePort
           │
    FuzzerRunPort (ports/inbound)           ← inbound contract
           │
    RunFuzzerUseCase (use_cases)            ← implements FuzzerRunPort, owns outbound port
           │
    TestRunnerPort (ports/outbound)         ← outbound contract
           │
    ForgeRunner (adapters/outbound)         ← implements TestRunnerPort, spawns forge
```

### Inbound adapter — `adapters/inbound/fuzzer.rs`

Implements `FuzzerEnginePort`. Holds `Box<dyn FuzzerRunPort>`. Delegates entirely to the use case — contains no logic of its own.

### Inbound port — `ports/inbound/fuzzer_run_port.rs`

```rust
pub trait FuzzerRunPort: Send + Sync {
    async fn run(&self, signal: RoundSignal) -> Result<FuzzReport>;
}
```

### Use case — `use_cases/run_fuzzer_session.rs`

`RunFuzzerUseCase` implements `FuzzerRunPort`. Owns the outbound `TestRunnerPort`:

```rust
pub struct RunFuzzerUseCase {
    pub runner: Box<dyn TestRunnerPort>,
}
```

Sequences the full round:
1. Run `forge test` via `run_fuzzer()`
2. Write stdout to `.fuzzming/fuzz_output.txt`
3. Evaluate outcome via `evaluate_outcome()`
4. If `Pass` — run `forge coverage` to generate `lcov.info`
5. Return `FuzzReport { outcome }`

### Outbound port — `ports/outbound/test_runner_port.rs`

```rust
pub trait TestRunnerPort: Send + Sync {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult>;
    async fn run_coverage(&self, profile_name: &str) -> Result<RunnerResult>;
}
```

### Outbound adapter — `adapters/outbound/forge_runner.rs`

`ForgeRunner` is the single process boundary — the only struct allowed to spawn forge. Uses `FOUNDRY_PROFILE` env var to select the Foundry profile:

```rust
tokio::process::Command::new("forge")
    .args(["test"])
    .env("FOUNDRY_PROFILE", profile_name)
    .current_dir(&self.working_dir)
    .output()
    .await
```

Both `run_test` and `run_coverage` capture stdout, stderr, and exit code into a `RunnerResult`.

---

## Outcome evaluation — `use_cases/evaluate_outcome.rs`

Pure function. Inspects `RunnerResult` and returns a `FuzzOutcome`:

| Condition | Outcome |
|---|---|
| `exit_code == 0` | `Pass` |
| Exit non-zero, a `[FAIL` line contains `invariant_` | `Bug` |
| Exit non-zero, no invariant `[FAIL` found | `DevTestFailed` |

Handles both forge output formats:
- `[FAIL. Reason: Invariant violation.] invariant_balance() (runs: 100)`
- `[FAIL: VaultInvariantTest::invariant_balance()] (runs: 256)`

Coverage (`forge coverage`) is only triggered on `Pass` — never on `Bug` or `DevTestFailed`.

---

## Data flow

```
Orchestrator
  │
  └─ Fuzzer::run(signal)                     ← FuzzerEnginePort (inbound adapter)
       │
       └─ RunFuzzerUseCase::run(signal)      ← FuzzerRunPort (use case)
             │
             ├─ run_fuzzer("fuzzming", runner)
             │     → forge test (FOUNDRY_PROFILE=fuzzming)
             │     → write stdout to .fuzzming/fuzz_output.txt
             │
             ├─ evaluate_outcome(&result)
             │     → FuzzOutcome: Pass | Bug | DevTestFailed
             │
             └─ if Pass:
                   run_coverage("coverage", runner)
                   → forge coverage (FOUNDRY_PROFILE=coverage)
                   → lcov.info written to workspace root by forge
```

---

## FuzzReport

```rust
pub struct FuzzReport {
    pub outcome: FuzzOutcome,
}

pub enum FuzzOutcome {
    Pass,
    Bug,
    FullCoverage,
    DevTestFailed,
}
```

`FullCoverage` is reserved for future use — the orchestrator will determine full coverage by reading `lcov.info` via the reader.

---

## Wiring at startup

```rust
let runner   = Box::new(ForgeRunner::new(workspace_root));
let use_case = Box::new(RunFuzzerUseCase::new(runner));
let fuzzer   = Fuzzer::new(use_case);
```

`Fuzzer` never imports `RunFuzzerUseCase`. All concrete types are resolved at the entry point only.

---

## Hard rules

- `Fuzzer` never writes Solidity files — that is the Executor's job.
- `Fuzzer` never calls the LLM — that is the Generator's job.
- `ForgeRunner` is the only struct that spawns forge subprocesses.
- Coverage is only run when `forge test` exits with code 0.
