# Reporter Component

The Reporter is the **output gateway** of FuzzMing. It receives a `SessionOutcome` (which already carries all fuzzing artifacts), formats a human-readable message, and writes it to one or more outputs (terminal or GitHub PR comment). It never runs forge, never calls the LLM, never writes Solidity files, and never reads from disk.

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
│       ├── terminal_output.rs           # TerminalOutput — prints to stdout
│       └── pr_comment_output.rs         # PrCommentOutput — posts to GitHub PR via REST API
├── ports/
│   └── outbound/
│       └── output_port.rs              # OutputPort — outbound contract for writing the final message
└── use_cases/
    ├── format_bug_report.rs            # formats Bug termination
    ├── format_coverage_report.rs       # formats FullCoverage termination
    ├── format_dev_test_failure.rs      # formats DevTestFailed termination
    └── format_exhausted_report.rs      # formats Exhausted termination
```

---

## Architecture layers

```
Orchestrator
    │  builds SessionOutcome (with artifacts already populated)
    │
    └─ ReporterPort (shared/ports)
           │
    Reporter (adapters/inbound)              ← implements ReporterPort
           │
           └─ OutputPort (ports/outbound)    ← outbound contract for writing the message
                   │
           TerminalOutput / PrCommentOutput  ← concrete output destinations
```

### Inbound adapter — `adapters/inbound/reporter.rs`

Implements `ReporterPort`. Holds `Box<dyn OutputPort>`. Dispatches to the matching formatter, then writes via `OutputPort`. Contains no reading logic.

### Shared port — `shared/ports/reporter_port.rs`

```rust
pub trait ReporterPort: Send + Sync {
    async fn emit(&self, outcome: SessionOutcome) -> Result<()>;
}
```

Called by the orchestrator at the end of every session, once per contract.

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
| `format_bug_report` | `TerminationReason::Bug` | `## FuzzMing: N bug(s) found in \`{contract}\` (round {n})` |
| `format_coverage_report` | `TerminationReason::FullCoverage` | `## FuzzMing: Full Coverage Achieved for \`{contract}\` (round {n})` |
| `format_dev_test_failure` | `TerminationReason::DevTestFailed` | `## FuzzMing: Forge Tests Failed for \`{contract}\` (round {n})` |
| `format_exhausted_report` | `TerminationReason::Exhausted` | `## FuzzMing: Rounds Exhausted for \`{contract}\` ({n} rounds, no bugs found)` |

**Bug report** renders one numbered block per failing invariant (`**Bug 1:**`, `**Bug 2:**`, …), each showing the invariant's call sequence from `outcome.artifacts.call_sequences`. If no bugs were captured, the block reads `(no call sequences captured)`. The raw forge output follows (truncated to 3 000 chars).

`call_sequences` is populated by the orchestrator from `FuzzReport.bugs`: each `BugInfo` becomes `"{invariant_name}:\n{call_sequence}"` — structured data from the fuzzer's state machine parser, not raw stdout.

**Coverage / Exhausted reports** include a coverage summary from `outcome.artifacts.coverage_summary`.

**DevTestFailed report** includes the raw forge output (truncated to 3 000 chars).

---

## Outbound adapters

### `TerminalOutput`

Prints the formatted message to stdout via `println!`. No configuration required.

### `PrCommentOutput`

Posts the formatted message as a GitHub PR comment via the REST API:

```
POST https://api.github.com/repos/{repo}/issues/{pr_number}/comments
Authorization: Bearer {github_token}
```

Requires three fields at construction time: `github_token`, `repo` (`owner/name`), `pr_number`.

---

## Data model

### `SessionOutcome` (input)

```rust
pub struct SessionOutcome {
    pub reason: TerminationReason,   // Bug | FullCoverage | DevTestFailed | Exhausted
    pub contract_name: String,
    pub rounds_completed: u32,
    pub artifacts: ReportArtifacts,
}
```

### `ReportArtifacts`

```rust
pub struct ReportArtifacts {
    pub fuzz_output: String,
    pub coverage_summary: Option<String>,  // None when lcov.info is absent
    pub call_sequences: Vec<String>,
}
```

`ReportArtifacts` is populated by the orchestrator:
- `fuzz_output` — read from `.fuzzming/{Contract}/fuzz_output.txt` via `ReaderPort::get_fuzz_output`
- `coverage_summary` — read from `.fuzzming/{Contract}/lcov.info` via `ReaderPort::get_coverage_context`
- `call_sequences` — mapped from `FuzzReport.bugs`: each `BugInfo` becomes `"{invariant_name}:\n{call_sequence}"`

---

## Data flow

```
Orchestrator
  │
  ├─ ReaderPort::get_fuzz_output(path)               ← existing reader component
  ├─ ReaderPort::get_coverage_context(path)          ← existing reader component
  │
  ├─ builds ReportArtifacts { fuzz_output, coverage_summary, call_sequences }
  ├─ builds SessionOutcome { reason, contract_name, rounds_completed, artifacts }
  │
  └─ Reporter::emit(outcome)                         ← ReporterPort
       │
       ├─ match outcome.reason → format_*(&outcome) → message: String
       │
       └─ OutputPort::write(&message)
             TerminalOutput  → println!
             PrCommentOutput → POST GitHub API
```

---

## Wiring at startup

```rust
// pick one output:
let output = Box::new(TerminalOutput::new());
// or:
let output = Box::new(PrCommentOutput::new(github_token, repo, pr_number));

let reporter = Reporter::new(output);
```

`Reporter` never imports `TerminalOutput` or `PrCommentOutput`. All concrete types are resolved at the entry point only.

---

## Hard rules

- `Reporter` never reads from disk — artifacts arrive pre-populated in `SessionOutcome`.
- `Reporter` never runs forge subprocesses — that is the Fuzzer's job.
- `Reporter` never calls the LLM — that is the Generator's job.
- `Reporter` never writes Solidity files — that is the Executor's job.
- Formatters are pure functions — no I/O, no side effects.
