# Reporter Component

The Reporter is the **output gateway** of FuzzMing. It receives a `SessionOutcome` (which already carries all fuzzing results), formats a human-readable message, and writes it to the terminal. It never runs forge, never calls the LLM, never writes Solidity files, and never reads from disk.

---

## Responsibility

One job: given a `SessionOutcome`, dispatch to the right formatter and emit the result. All formatting logic lives in pure functions. The only I/O the reporter performs is writing the final message via `OutputPort`.

---

## Directory structure

```
src/reporter/
├── adapters/
│   ├── inbound/
│   │   └── reporter.rs                  # Inbound adapter — implements ReporterPort, wires formatters + output
│   └── outbound/
│       └── terminal_output.rs           # TerminalOutput — prints to stdout
├── ports/
│   └── outbound/
│       └── output_port.rs              # OutputPort — outbound contract for writing the final message
└── use_cases/
    ├── format_bug_report.rs            # formats Bug termination
    ├── format_compile_error.rs         # formats CompileError termination
    ├── format_coverage_report.rs       # formats FullCoverage termination
    ├── format_dev_test_failure.rs      # formats DevTestFailed termination
    ├── format_exhausted_report.rs      # formats Exhausted termination
    └── format_round_usage.rs           # shared helper — formats per-round LLM token usage
```

---

## Architecture layers

```
Orchestrator
    │  builds SessionOutcome (bugs + coverage_snapshots already populated)
    │
    └─ ReporterPort (shared/ports)
           │
    Reporter (adapters/inbound)              ← implements ReporterPort
           │
           └─ OutputPort (ports/outbound)    ← outbound contract for writing the message
                   │
           TerminalOutput                    ← prints to stdout
```

### Inbound adapter — `adapters/inbound/reporter.rs`

Implements `ReporterPort`. Holds `Box<dyn OutputPort>`. Dispatches to the matching formatter, then writes via `OutputPort`. Contains no reading logic.

### Shared port — `shared/ports/reporter_port.rs`

```rust
pub trait ReporterPort: Send + Sync {
    async fn emit(&self, outcome: SessionOutcome) -> Result<()>;
}
```

Called by the orchestrator at the end of each contract's session, once per contract.

### Outbound port — `ports/outbound/output_port.rs`

```rust
pub trait OutputPort: Send + Sync {
    async fn write(&self, output: &str) -> Result<()>;
}
```

---

## Use cases (formatters)

Each formatter is a pure function `fn(&SessionOutcome) -> String`. No I/O, no side effects.

| Formatter | Trigger | Headline |
|---|---|---|
| `format_bug_report` | `TerminationReason::Bug` | `## FuzzMing: N bug(s) found in \`{contract}\`` |
| `format_compile_error_outcome` | `TerminationReason::CompileError` | `## FuzzMing: Compile Error — \`{contract}\` never ran` |
| `format_coverage_report` | `TerminationReason::FullCoverage` | `## FuzzMing: Full Coverage Achieved for \`{contract}\`` |
| `format_dev_test_failure` | `TerminationReason::DevTestFailed` | `## FuzzMing: Forge Tests Failed for \`{contract}\`` |
| `format_exhausted_report` | `TerminationReason::Exhausted` | `## FuzzMing: Rounds Exhausted for \`{contract}\` ({n} rounds, X bugs found)` |

**Bug report** renders one numbered block per failing invariant (`**Bug 1:**`, `**Bug 2:**`, …), each showing the invariant name and call sequence from `outcome.bugs`. If no bugs were captured, the block reads `(no call sequences captured)`.

**CompileError report** explains that the generated test code never compiled and the contract was never exercised. The raw compiler error is included (truncated to 3 000 chars) so the problem is visible without digging into `.fuzzming/`.

**Exhausted report** shows a count and bulleted list of every bug found across all rounds using `outcome.bugs`. If no bugs were found the summary reads "no bugs found"; otherwise it reads "X bugs found" with one `- \`invariant_name\`` line per bug. Coverage snapshots from `outcome.coverage_snapshots` are included when present.

**Coverage report** includes the coverage snapshot summary.

**DevTestFailed report** includes the raw forge output (truncated to 3 000 chars).

### `format_round_usage`

A shared helper used by `format_exhausted_report` and `format_coverage_report` to render per-round LLM token usage when the generator returned usage data.

---

## Outbound adapter — `TerminalOutput`

Prints the formatted message to stdout via `println!`. No configuration required.

Previously the reporter also had a `PrCommentOutput` adapter for posting results as GitHub PR comments. This was removed — CI integration is now handled by checking the exit code (exit 1 when bugs are found) rather than posting comments.

---

## Data model

### `SessionOutcome` (input)

```rust
pub struct SessionOutcome {
    pub reason: TerminationReason,   // Bug | FullCoverage | DevTestFailed | Exhausted | CompileError
    pub contract_name: String,
    pub rounds_completed: u32,
    pub bugs: Vec<BugInfo>,          // all bugs found across all rounds
    pub coverage_snapshots: Vec<String>, // per-round coverage summary strings
}
```

`bugs` carries every `BugInfo` accumulated across all rounds. The `Bug` report uses `bugs` to render one block per failing invariant. The `Exhausted` report uses `bugs` to show a count and list even when the session ran to completion without a definitive `Bug` termination reason.

`coverage_snapshots` is populated by the orchestrator with one string per round that produced a passing `forge coverage` result.

---

## Data flow

```
Orchestrator
  │
  ├─ accumulates BugInfo across rounds → outcome.bugs
  ├─ accumulates coverage strings     → outcome.coverage_snapshots
  │
  └─ Reporter::emit(outcome)                          ← ReporterPort
       │
       ├─ match outcome.reason → format_*(&outcome) → message: String
       │
       └─ OutputPort::write(&message)
             TerminalOutput  → println!
```

---

## Session summary (CLI runner)

After `orchestrator.run()` returns all outcomes, `CliRunner::print_aggregate_summary` prints a coloured session summary to the terminal — this is not part of the Reporter component. It is rendered by the entry point after all per-contract reports have been emitted.

The summary shows:
- Total contracts, passed, with bugs, not tested
- Compile errors and setup failures as sub-counts of "not tested"
- Total rounds and total bugs across all contracts

---

## Wiring at startup

```rust
let output   = Box::new(TerminalOutput::new());
let reporter = Box::new(Reporter::new(output));
```

`Reporter` never imports `TerminalOutput`. All concrete types are resolved at the entry point only.

---

## Hard rules

- `Reporter` never reads from disk — all data arrives pre-populated in `SessionOutcome`.
- `Reporter` never runs forge subprocesses — that is the Fuzzer's job.
- `Reporter` never calls the LLM — that is the Generator's job.
- `Reporter` never writes Solidity files — that is the Executor's job.
- Formatters are pure functions — no I/O, no side effects.
