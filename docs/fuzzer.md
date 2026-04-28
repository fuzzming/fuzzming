# Fuzzer Component

The Fuzzer is the **execution gateway** of FuzzMing. It runs `forge test` against all generated invariant tests in one process, filters output per contract, and — if any contract passes — runs `forge coverage` and filters the lcov file per contract. It never writes Solidity files and never calls the LLM.

---

## Responsibility

One job: given a batch of `RoundSignal`s (one per contract), run forge once and return a `Vec<FuzzReport>` (one per contract). All subprocess logic lives in the outbound adapter — the use case orchestrates, filters, and evaluates.

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
    async fn run(&self, signals: Vec<RoundSignal>) -> Result<Vec<FuzzReport>>;
}
```

One call covers all contracts. The returned `Vec<FuzzReport>` is in the same order as the input `Vec<RoundSignal>`.

### Use case — `use_cases/run_fuzzer_session.rs`

`RunFuzzerUseCase` implements `FuzzerRunPort`. Owns the outbound `TestRunnerPort`:

```rust
pub struct RunFuzzerUseCase {
    pub runner: Box<dyn TestRunnerPort>,
}
```

Sequences the full round:
1. Guard against empty signals.
2. Run `forge test` once — covers all contracts.
3. For each contract: create `.fuzzming/{Contract}/`, filter stdout, write `fuzz_output.txt`, evaluate outcome.
4. If any contract passed: run `forge coverage` once, read `lcov.info` with `if let Ok(...)` (tolerates missing file), filter per contract by `SF:` lines, write `.fuzzming/{Contract}/lcov.info`, set `FuzzReport.lcov_path`.
5. Return `Vec<FuzzReport>`.

### Outbound port — `ports/outbound/test_runner_port.rs`

```rust
pub trait TestRunnerPort: Send + Sync {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult>;
    async fn run_coverage(&self, profile_name: &str) -> Result<RunnerResult>;
}
```

### Outbound adapter — `adapters/outbound/forge_runner.rs`

`ForgeRunner` is the single process boundary — the only struct allowed to spawn forge. Profile is selected via `FOUNDRY_PROFILE` env var (forge 1.x has no `--profile` flag):

```rust
tokio::process::Command::new("forge")
    .args(["test"])
    .env("FOUNDRY_PROFILE", profile_name)
    .current_dir(&self.working_dir)  // working_dir: PathBuf
    .output()
    .await
```

Both `run_test` and `run_coverage` capture stdout, stderr, and exit code into a `RunnerResult`.

---

## Outcome evaluation — inline in `run_fuzzer_session.rs`

`evaluate_outcome_for_contract` scans the combined stdout + stderr for `[FAIL` lines that mention the contract's invariant test name:

| Condition | Outcome |
|---|---|
| No `[FAIL` line for this contract | `Pass` |
| `[FAIL` line contains `invariant_` | `Bug` |
| `[FAIL` line present but no `invariant_` | `DevTestFailed` |

Coverage (`forge coverage`) is only triggered when at least one contract's outcome is `Pass`.

---

## Per-contract output filtering

All contracts run in one forge process. After the process exits, Rust filters the output:

**`filter_output_for_contract`** — keeps only stdout lines that contain `{Contract}InvariantTest`. Written to `.fuzzming/{Contract}/fuzz_output.txt`.

**`filter_lcov_for_contract`** — walks `lcov.info` line by line; keeps each `SF:` block only if the `SF:` path contains the contract name. Written to `.fuzzming/{Contract}/lcov.info`.

---

## File system layout (fuzzer-owned paths)

```
{workspace_root}/
├── lcov.info                             ← forge writes here (temporary, overwritten each round)
└── .fuzzming/
    └── {ContractName}/
        ├── fuzz_output.txt               ← filtered forge test stdout
        └── lcov.info                     ← filtered forge coverage output
```

The root `lcov.info` is forge's raw output. Per-contract `lcov.info` files are filtered copies.

---

## Data flow

```
Orchestrator
  │
  └─ Fuzzer::run(signals)                         ← FuzzerEnginePort (inbound adapter)
       │
       └─ RunFuzzerUseCase::run(signals)           ← FuzzerRunPort (use case)
             │
             ├─ run_fuzzer("fuzzming", runner)
             │     FOUNDRY_PROFILE=fuzzming forge test
             │     → RunnerResult { stdout, stderr, exit_code }
             │
             ├─ for each contract:
             │     filter stdout → .fuzzming/{Contract}/fuzz_output.txt
             │     evaluate_outcome_for_contract() → FuzzOutcome
             │
             └─ if any_pass:
                   run_coverage("coverage", runner)
                   FOUNDRY_PROFILE=coverage forge coverage --report lcov
                   if lcov.info exists:
                     for each passing contract:
                       filter lcov → .fuzzming/{Contract}/lcov.info
                       FuzzReport.lcov_path = Some(path)
```

---

## `FuzzReport`

```rust
pub struct FuzzReport {
    pub outcome: FuzzOutcome,
    pub lcov_path: Option<PathBuf>,  // set only on Pass; None otherwise
}

pub enum FuzzOutcome {
    Pass,
    Bug,
    FullCoverage,   // reserved — not yet returned
    DevTestFailed,
}
```

`lcov_path` carries the absolute path to `.fuzzming/{Contract}/lcov.info`. The orchestrator passes this to the reader for the next round's `CoverageContext`.

---

## Wiring at startup

```rust
let runner   = Box::new(ForgeRunner::new(workspace_root)); // PathBuf
let use_case = Box::new(RunFuzzerUseCase::new(runner));
let fuzzer   = Fuzzer::new(use_case);
```

`Fuzzer` never imports `RunFuzzerUseCase`. All concrete types are resolved at the entry point only.

---

## Hard rules

- `Fuzzer` never writes Solidity files — that is the Executor's job.
- `Fuzzer` never calls the LLM — that is the Generator's job.
- `ForgeRunner` is the only struct that spawns forge subprocesses.
- Coverage is only run when at least one contract outcome is `Pass`.
- `lcov.info` missing after coverage is silently tolerated — `lcov_path` stays `None`.

---

## Known issues

1. **Exit-code blindness** — `evaluate_outcome_for_contract` returns `Pass` if no `[FAIL]` line mentions the contract, even when forge exited non-zero. Should return `DevTestFailed` when `exit_code != 0` and no contract output is found.

2. **Incomplete call-sequence capture** — `filter_output_for_contract` only keeps lines containing the contract name. Forge's failing call sequence spans multiple lines that don't repeat the contract name, so `fuzz_output.txt` misses most of the sequence.

3. **`FullCoverage` never returned** — no logic detects 100% coverage from lcov data and promotes the outcome.
