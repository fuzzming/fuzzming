# Fuzzer Component

The Fuzzer is the **execution gateway** of FuzzMing. It runs `forge test` against all generated invariant tests in one process, filters output per contract, and, if any contract passes, runs `forge coverage` and filters the lcov file per contract. It never writes Solidity files and never calls the LLM.

---

## Contents

- [Responsibility](#responsibility)
- [Directory structure](#directory-structure)
- [Architecture layers](#architecture-layers)
- [Outcome evaluation](#outcome-evaluation)
- [File system layout](#file-system-layout-fuzzer-owned-paths)
- [Data flow](#data-flow)
- [`FuzzReport`](#fuzzreport)
- [Wiring at startup](#wiring-at-startup)
- [Hard rules](#hard-rules)
- [Known issues](#known-issues)

---

## Responsibility

One job: given a batch of `RoundSignal`s (one per contract), run forge once and return a `Vec<FuzzReport>` (one per contract). All subprocess logic and output parsing live in the outbound adapter: the use case only orchestrates.

---

## Directory structure

```
src/fuzzer/
├── adapters/
│   ├── inbound/
│   │   └── fuzzer.rs                       # Inbound adapter: implements FuzzerEnginePort, delegates to FuzzerRunPort
│   └── outbound/
│       ├── forge_runner.rs                 # ForgeRunner: spawns forge, parses forge output, reads lcov.info
│       └── file_system_fuzzer_output.rs    # FileSystemFuzzerOutput: writes fuzz_output.txt and lcov.info
├── ports/
│   ├── inbound/
│   │   └── fuzzer_run_port.rs              # FuzzerRunPort: inbound contract between adapter and use case
│   └── outbound/
│       ├── test_runner_port.rs             # TestRunnerPort: outbound contract for running forge and parsing its output
│       └── fuzzer_output_port.rs           # FuzzerOutputPort: outbound contract for writing per-contract files
└── use_cases/
    ├── run_fuzzer_session.rs               # RunFuzzerUseCase: implements FuzzerRunPort, owns both outbound ports; contains private evaluate_outcome_for_contract
    ├── run_fuzzer.rs                       # Pre-flight forge build check, then run_test()
    └── run_coverage.rs                     # Thin wrapper: calls runner.run_coverage()
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
    RunFuzzerUseCase (use_cases)            ← implements FuzzerRunPort, owns both outbound ports
           │
    ┌──────┴──────────────────────┐
    │                             │
    TestRunnerPort                FuzzerOutputPort
    (ports/outbound)              (ports/outbound)
           │                             │
    ForgeRunner               FileSystemFuzzerOutput
    (adapters/outbound)        (adapters/outbound)
```

### Inbound adapter: `adapters/inbound/fuzzer.rs`

Implements `FuzzerEnginePort`. Holds `Box<dyn FuzzerRunPort>`. Delegates entirely to the use case: contains no logic of its own.

### Inbound port: `ports/inbound/fuzzer_run_port.rs`

```rust
pub trait FuzzerRunPort: Send + Sync {
    async fn run(&self, signals: Vec<RoundSignal>) -> Result<Vec<FuzzReport>>;
}
```

One call covers all contracts. The returned `Vec<FuzzReport>` is in the same order as the input `Vec<RoundSignal>`.

### Use case: `use_cases/run_fuzzer_session.rs`

`RunFuzzerUseCase` implements `FuzzerRunPort`. Owns both outbound ports:

```rust
pub struct RunFuzzerUseCase {
    pub runner: Box<dyn TestRunnerPort>,
    pub output: Box<dyn FuzzerOutputPort>,
}
```

Sequences the full round:
1. Guard against empty signals.
2. Run `forge test` once: covers all contracts.
3. For each contract: ask `runner` to filter stdout and collect bugs; ask `output` to write `fuzz_output.txt`; evaluate outcome.
4. If any contract passed: run `forge coverage` once; the runner returns raw lcov content via `CoverageResult`; filter per contract; write per-contract `lcov.info`; parse and enrich coverage gaps with nearby source lines; write `.fuzzming/{Contract}/coverage_context.json`; set `FuzzReport.lcov_path`.
5. Return `Vec<FuzzReport>`.

The use case contains no forge-specific parsing and performs no filesystem I/O directly.

### Outbound port: `ports/outbound/test_runner_port.rs`

```rust
pub trait TestRunnerPort: Send + Sync {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult>;
    async fn run_coverage(&self, profile_name: &str) -> Result<CoverageResult>;

    fn collect_bugs(&self, stdout: &str, contract_name: &str) -> Vec<BugInfo>;
    fn filter_output(&self, stdout: &str, contract_name: &str) -> String;
    fn filter_lcov(&self, lcov: &str, contract_name: &str) -> String;
}
```

The three synchronous methods carry all knowledge of forge's output format. They are defined on the port so the use case can delegate to them without knowing which runner is in use.

`CoverageResult` carries:

```rust
pub struct CoverageResult {
    pub exit_code: i32,
    pub lcov_content: Option<String>,  // None if lcov.info was not written
}
```

### Outbound port: `ports/outbound/fuzzer_output_port.rs`

```rust
pub trait FuzzerOutputPort: Send + Sync {
    async fn write_fuzz_output(&self, contract_name: &str, content: &str) -> Result<()>;
    async fn write_lcov(&self, contract_name: &str, content: &str) -> Result<PathBuf>;
    async fn write_coverage_context(&self, contract_name: &str, context: &CoverageContext) -> Result<()>;
}
```

Returns the absolute path to the written `lcov.info` so the use case can populate `FuzzReport.lcov_path`.

### Outbound adapter: `adapters/outbound/forge_runner.rs`

`ForgeRunner` is the single process boundary: the only struct allowed to spawn forge. Profile is selected via `FOUNDRY_PROFILE` env var (forge 1.x has no `--profile` flag). The `[profile.fuzzming]` section written by the Executor sets `test = "test/fuzzming"`, so forge automatically scopes itself to the generated tests with no CLI flags:

```rust
// Pre-flight compile check: fast fail before spending the test run budget
tokio::process::Command::new("forge")
    .args(["build"])             // test = "test/fuzzming" from foundry.toml profile
    .env("FOUNDRY_PROFILE", profile_name)
    .current_dir(&self.working_dir)
    .output()
    .await

// Full invariant test run (only reached when build passes)
tokio::process::Command::new("forge")
    .args(["test"])              // test = "test/fuzzming" from foundry.toml profile
    .env("FOUNDRY_PROFILE", profile_name)
    .current_dir(&self.working_dir)
    .output()
    .await
```

`run_build` and `run_test` both capture stdout, stderr, and exit code into `RunnerResult`.

`run_coverage` spawns `forge coverage --report lcov`, then reads `lcov.info` from `self.working_dir` and returns the raw content in `CoverageResult.lcov_content` (or `None` if the file was not written).

`ForgeRunner` also owns all forge-specific output parsing:

#### `collect_bugs`: forge output state machine

Forge reports each failing invariant with a multi-line block. Both tokens are **always on separate lines** (empirically verified):

```
[FAIL: assertion message]
  [Sequence] (original: N, shrunk: M)
    sender=0x... calldata=handler_reset() args=[]
 invariant_never_zero() (runs: 1, calls: 1, reverts: 0)
```

The reliable signal for a failing invariant is a line containing both `invariant_` and `(runs:`. The `[Sequence]` block immediately above it is the reproduction call sequence.

The state machine walks stdout line by line:

1. Enters the contract's section when a line contains `{Contract}InvariantTest`
2. Exits if a different `InvariantTest` header appears (multi-contract output)
3. On `[FAIL`: opens a new block, clears the sequence buffer
4. On `[Sequence]` or `[Shrunk sequence]`: starts collecting sequence lines
5. On box-drawing characters (`╭`, `╰`, `├`): sequence collection ends (call stats table)
6. On `invariant_` + `(runs:`: saves a `BugInfo`, closes the block; a `HashSet` deduplicates by name
7. Stops at `Failing tests:`: forge prints results twice; this prevents duplicates

Returns `Vec<BugInfo>`: one entry per unique failing invariant, each carrying `invariant_name` and `call_sequence`.

#### `filter_output`

Captures the full output section for one contract: starts when a line contains `{Contract}InvariantTest`, stops when the next `InvariantTest` header or `Failing tests:` appears. Preserves the complete multi-line fail blocks (call sequences, stats) rather than just lines that happen to mention the contract name.

#### `filter_lcov`

Walks raw `lcov.info` text line by line; keeps each `SF:` block only if the `SF:` path contains the contract name.

### Outbound adapter: `adapters/outbound/file_system_fuzzer_output.rs`

`FileSystemFuzzerOutput` is the only struct that performs filesystem writes for the fuzzer. It owns the `.fuzzming/{Contract}/` path convention:

```rust
pub struct FileSystemFuzzerOutput {
    workspace_root: PathBuf,
}
```

All methods create the per-contract directory if it does not exist before writing. In addition to `fuzz_output.txt` and `lcov.info`, the adapter writes `coverage_context.json` after the fuzzer enriches coverage gaps with nearby source lines.

---

## Outcome evaluation

`RunFuzzerUseCase` first checks whether forge's output indicates a **compilation error** (exit code non-zero and stderr/stdout contains `Compiler run failed` or `error[`). Forge emits detailed solc errors to stdout, so the compile-error message prefers stdout when it contains `Error (` or `TypeError` and falls back to stderr otherwise.

If only some contracts fail to compile, the use case temporarily moves their `test/fuzzming/<Contract>/` directories to `.fuzzming-disabled/`, re-runs forge for the healthy contracts, and restores the stashed dirs before returning. The erroring contracts receive `CompileError` with the compiler output in `fuzz_output.txt` so the LLM can repair them next round.

Otherwise `evaluate_outcome_for_contract` delegates bug collection to the runner port:

| Condition | Outcome | Bugs |
|---|---|---|
| Compilation error detected | `CompileError` | `[]` |
| Exit code 0 | `Pass` | `[]` |
| Exit code non-zero, `runner.collect_bugs()` returns non-empty | `Bug` | one `BugInfo` per failing invariant |
| Exit code non-zero, `runner.collect_bugs()` returns empty | `DevTestFailed` | `[]` |

`CompileError` and `DevTestFailed` do **not** terminate the session: the orchestrator continues to the next round so the LLM can fix the generated Solidity.

**Setup failure detection:** forge can exit 0 even when `setUp()` reverts and every invariant runs with 0 calls. When that happens, the outcome is coerced to `CompileError` and the fuzzer writes a `SETUP FAILURE` message to `fuzz_output.txt` so the LLM can fix constructor mocks and `targetContract` wiring.

**`DevTestFailed` output capture**: `filter_output` looks for the `{Contract}InvariantTest` section header in forge stdout. For `DevTestFailed` (setUp revert, runtime panic, unused variable error), the error appears outside that section and `filter_output` returns an empty string. In that case the fuzzer falls back to writing the full `stderr + stdout` with a `"TEST FAILED: fix the handler/invariant test:"` header, so the LLM receives the actual error rather than an empty feedback.

Coverage (`forge coverage`) is only triggered when at least one contract's outcome is `Pass`.

---

## File system layout (fuzzer-owned paths)

```
{workspace_root}/
├── lcov.info                             ← forge writes here; ForgeRunner reads it after coverage
└── .fuzzming/
    └── {ContractName}/
        ├── fuzz_output.txt               ← filtered forge test stdout (written by FileSystemFuzzerOutput)
        ├── lcov.info                     ← filtered forge coverage output (written by FileSystemFuzzerOutput)
        ├── coverage_context.json         ← enriched coverage gaps (written by FileSystemFuzzerOutput)
        ├── {Contract}.bodies.json        ← LLM bodies artifact (written by Executor)
        ├── {Contract}.config.json        ← fuzzer config artifact (written by Executor)
        └── outcome.json                  ← final session outcome (written by Orchestrator)
```

The root `lcov.info` is forge's raw output; it is read by `ForgeRunner` and never accessed by the use case. Per-contract `lcov.info` files are filtered copies written by `FileSystemFuzzerOutput`.
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
             │     FOUNDRY_PROFILE=fuzzming forge build  (pre-flight; aborts if compile error)
             │     FOUNDRY_PROFILE=fuzzming forge test   (scoped to test/fuzzming via profile)
             │     → RunnerResult { stdout, stderr, exit_code }
             │
             ├─ for each contract:
             │     runner.filter_output(stdout, contract) → full section block
             │       → output.write_fuzz_output(contract, block)
             │           → .fuzzming/{Contract}/fuzz_output.txt
             │     runner.collect_bugs(stdout, contract) → Vec<BugInfo>
             │     evaluate_outcome_for_contract() → (FuzzOutcome, Vec<BugInfo>)
             │     FuzzReport { outcome, bugs, lcov_path: None }
             │
             └─ if any_pass:
                   run_coverage("coverage", runner)
                   FOUNDRY_PROFILE=coverage forge coverage --report lcov
                   ForgeRunner reads lcov.info → CoverageResult { lcov_content: Some(...) }
                   for each passing contract:
                     runner.filter_lcov(lcov_content, contract) → filtered string
                     output.write_lcov(contract, filtered) → PathBuf
                                         parse_lcov(filtered) → CoverageContext
                                         enrich_coverage_context(...) → add source_context
                                         output.write_coverage_context(contract, context)
                                         FuzzReport.lcov_path = Some(path)
```

---

## `FuzzReport`

```rust
pub struct FuzzReport {
    pub outcome: FuzzOutcome,
    /// All failing invariants found in this forge run.
    /// Populated only when outcome == Bug; empty otherwise.
    pub bugs: Vec<BugInfo>,
    /// Path to the lcov.info file written by FileSystemFuzzerOutput.
    /// Set only when outcome is Pass or FullCoverage; None otherwise.
    pub lcov_path: Option<PathBuf>,
}

pub enum FuzzOutcome {
    Pass,
    Bug,
    FullCoverage,   // reserved: not yet returned
    CompileError,   // forge could not compile; LLM repairs next round
    DevTestFailed,
}

pub struct BugInfo {
    pub invariant_name: String,  // e.g. "invariant_never_zero"
    pub call_sequence: String,   // multi-line forge shrunk call sequence
}
```

`bugs` carries all failing invariants found by `runner.collect_bugs()`. The orchestrator maps each `BugInfo` into a formatted call-sequence string in `ReportArtifacts.call_sequences`.

`lcov_path` carries the absolute path to `.fuzzming/{Contract}/lcov.info`. The orchestrator passes this to the reader for the next round's `CoverageContext`.

---

## Wiring at startup

```rust
let runner   = Box::new(ForgeRunner::new(workspace_root.clone()));
let output   = Box::new(FileSystemFuzzerOutput::new(workspace_root));
let use_case = Box::new(RunFuzzerUseCase::new(runner, output));
let fuzzer   = Fuzzer::new(use_case);
```

`Fuzzer` never imports `RunFuzzerUseCase`, `ForgeRunner`, or `FileSystemFuzzerOutput`. All concrete types are resolved at the entry point only.

---

## Hard rules

- `Fuzzer` never writes Solidity files: that is the Executor's job.
- `Fuzzer` never calls the LLM: that is the Generator's job.
- `ForgeRunner` is the only struct that spawns forge subprocesses.
- `ForgeRunner` is the only struct that knows forge's output format.
- `FileSystemFuzzerOutput` is the only struct that performs filesystem writes for the fuzzer.
- The use case contains no forge-specific parsing and no direct filesystem I/O.
- Forge is always scoped to `test/fuzzming/` via the profile's `test` key: no `--match-path` flag is ever passed.
- `run_fuzzer.rs` runs `forge build` before `forge test`; if the build fails the test is skipped and the compile error is returned immediately.
- Coverage is only run when at least one contract outcome is `Pass`: not on Bug, DevTestFailed, or CompileError rounds.
- Missing `lcov.info` after coverage is silently tolerated: `CoverageResult.lcov_content` is `None`, `lcov_path` stays `None`.
- For `DevTestFailed` with empty filtered output, the fuzzer writes full `stderr + stdout` so the LLM always receives actionable error context.

---

## Known issues

1. **`FullCoverage` never returned**: no logic detects 100% coverage from lcov data and promotes the outcome to `FullCoverage`.
