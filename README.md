# FuzzMing

FuzzMing is a Solidity smart contract fuzzing assistant. It closes the loop between LLM-generated invariants and Foundry's fuzzer — generating tests, running them, reading the results, and iterating until it finds a bug, achieves full coverage, or exhausts the configured round budget.

---

## How it works

Each round follows this sequence:

```
1. LLMEngine reads the target contract(s) and any previous fuzz output / coverage gaps
2. LLMEngine assembles a prompt and calls the configured LLM endpoint
3. LLMEngine writes generated invariants → test/fuzzming/*.fuzzming.t.sol
4. LLMEngine writes an adapted Foundry config → foundry.toml [profile.fuzzming]
5. FuzzerEngine runs test runner: forge test --profile fuzzming
6. If runner exits 0 → FuzzerEngine runs: forge coverage --profile coverage
7. Orchestrator evaluates the outcome and either stops or starts the next round
8. Reporter formats and emits the final result
```

Termination happens on the first of these conditions:

| Condition | What gets reported |
|---|---|
| Bug confirmed (invariant falsified) | Falsified invariant + call sequence + exact values |
| Developer test failed | Test name + assertion message |
| Full coverage reached | Coverage summary |
| Rounds exhausted | Uncovered lines with ±3 lines of source context |

---

## Architecture

FuzzMing is built around a strict component boundary model. The Orchestrator sequences work but contains no logic. Each component is self-sufficient and communicates only through typed signals.

```
SessionOrchestrator
    │
    ├── LLMEngine          generates invariants and Foundry config
    ├── FuzzerEngine       runs test runner (forge by default), evaluates outcomes
    ├── Reader             single read gateway (fs.read only)
    ├── Executor           single write gateway (fs.write only)
    └── Reporter           stateless formatter and emitter
```

**Hard rules:**
- No component calls another component directly
- `Reader` never writes. `Executor` never reads.
- `Executor` never touches developer-owned files (`test/*.t.sol`, `test/*.invariants.t.sol`)
- `CompositionRoot` is the only file that imports concrete infrastructure types
- Port interfaces belong to the component that defines what it needs

### File ownership

| Path | Owner | FuzzMing action |
|---|---|---|
| `test/*.t.sol` | Developer | Read as context only — never modified |
| `test/*.invariants.t.sol` | Developer | Read to avoid duplication — never modified |
| `test/fuzzming/*.fuzzming.t.sol` | FuzzMing | Created and overwritten each round |
| `foundry.toml [profile.fuzzming]` | FuzzMing | Written after every LLM round |
| `foundry.toml [profile.coverage]` | FuzzMing | Written once on first round |
| `fuzz_output.txt` | forge subprocess | Read by Reader for LLMEngine and Reporter |
| `lcov.info` | forge subprocess | Read by Reader for coverage analysis |
| `.fuzzming/memory.json` | FuzzMing | Appended after every confirmed bug finding |

---

## Source layout

```
src/
├── interfaces/          Shared data types — signals, contexts, artifacts, state
├── orchestrator/        Session loop + termination logic
├── llm/                 Prompt assembly, invariant generation, config adaptation
├── fuzzer/              test runner execution and outcome evaluation (forge by default, swappable via TestRunnerPort)
├── reader/              Single read gateway + all file parsers
├── executor/            Single write gateway + all file writers
├── reporter/            Stateless report formatter (terminal and CI output)
├── entry/               CLI (clap) and CI/CD (env vars) entry points
└── composition/         Composition root — the only file wiring concrete types
```

---

## Dependencies

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime |
| `async-trait` | Async methods on traits |
| `anyhow` | Error propagation |
| `serde` / `serde_json` | Serialization of artifacts and memory |
| `reqwest` | HTTP calls to LLM endpoints |
| `clap` | CLI argument parsing |

---

## Usage

### Prerequisites

- Rust (stable, 2021 edition)
- [Foundry](https://getfoundry.sh) installed and `forge` on `PATH`
- An OpenAI-compatible LLM endpoint

### Run

```bash
cargo build --release

./target/release/fuzzming \
  --targets src/MyContract.sol \
  --max-rounds 10 \
  --llm-url https://api.openai.com/v1 \
  --llm-key sk-...
```

Environment variables are also accepted for sensitive values:

```bash
export LLM_URL=https://api.openai.com/v1
export LLM_KEY=sk-...

./target/release/fuzzming --targets src/MyContract.sol --max-rounds 10
```

### CI/CD mode

Pass `--ci-mode` to emit structured output suitable for GitHub/GitLab PR comments instead of terminal formatting.

```bash
./target/release/fuzzming --targets src/MyContract.sol --ci-mode
```

---

## Development status

This repository contains the full architectural scaffolding. Component implementations are stubs (`todo!()`). The structure, all trait contracts, and data types are in place and the project compiles cleanly.

Implementation order that respects the dependency graph:

1. `Reader` + `FileSystemReader` — no external dependencies
2. `Executor` + `FileSystemWriter` — no external dependencies
3. `LLMEngine` (OpenAI/Anthropic adapters, parsers, use cases)
4. `FuzzerEngine` (`ForgeRunner`, outcome evaluation)
5. `Reporter` (format use cases, terminal and CI output)
6. `SessionOrchestrator` (wire everything via `CompositionRoot`)
