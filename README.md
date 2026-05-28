<div align="center">
  <img src="assets/fuzzming-logo.png" alt="FuzzMing" width="200" />
  <h1>FuzzMing</h1>
  <p><strong>LLM-powered fuzzing assistant for any language, any fuzzer</strong></p>
  <p>Point it at a project. It thinks, it fuzzes, it finds bugs.</p>
</div>

---

FuzzMing is an open source tool that closes the loop between an LLM and a fuzzer. It generates test contracts, runs them, reads the output, and iterates round after round until it finds every bug, achieves full coverage, or exhausts its round budget.

**Current stack: Solidity + Foundry.** The first supported target is Solidity smart contracts fuzzed with Foundry. But FuzzMing is not a Foundry tool, it is built on hexagonal architecture specifically so that new languages and fuzzers plug in as adapters without touching the core. Rust + cargo-fuzz, Vyper + Echidna, Move + any fuzzer: each is a set of adapters away. The orchestrator, session loop, LLM integration, and report format are language and fuzzer agnostic.

---

## What FuzzMing offers

- **Zero boilerplate:** give it a `.sol` file, it generates the full handler + invariant test suite from scratch
- **Continuous audit:** bugs don't stop the session, FuzzMing strips broken invariants, keeps hunting, and accumulates every finding across all rounds
- **Multi-contract sessions:** target multiple contracts in one run, each gets its own concurrent fuzzing lane
- **Any capable LLM:** OpenRouter, Groq, OpenAI, Anthropic, one flag switches providers
- **Compile error recovery:** a pre-flight `forge build` catches compile errors immediately before the full test run; the error is fed back to the LLM and retried next round
- **Isolated test execution:** the `[profile.fuzzming]` section in `foundry.toml` sets `test = "test/fuzzming"` so forge only runs FuzzMing-generated tests — your existing suite is never touched
- **Bug deduplication:** each unique breaking invariant is recorded once regardless of how many rounds it fires; the final report is never inflated with duplicates
- **Invariant code in reports:** every confirmed finding includes the full Solidity invariant function alongside the shrunk call sequence — drop it directly into a Foundry regression test
- **Coverage feedback:** after each passing round, LCOV coverage gaps are fed back to the LLM so it writes better invariants next time
- **Iterative security analysis:** patch rounds include a dedicated LLM audit pass that reviews fuzz output + confirmed bugs and prints a clean findings summary at the end of the session
- **Interactive or headless:** guided prompts for first-time users, `--defaults` / `--from-config` for CI pipelines
- **Non-destructive config patching:** only updates the fuzzming profiles in `foundry.toml`, preserving the rest of your config
- **Demo mode:** `fuzzming run --demo` runs the full UI with mock adapters, no LLM calls, no tokens spent

---

## Prerequisites

| Requirement | Install |
|---|---|
| Rust stable (2021 edition) | [rustup.rs](https://rustup.rs) |
| Foundry (`forge`) — required for the Solidity stack | `curl -L https://foundry.paradigm.xyz \| bash` |
| An LLM API key | OpenRouter, Groq, OpenAI, or Anthropic |

---

## Install

```bash
cargo install fuzzming
```

Or build from source:

```bash
git clone https://github.com/AchrefHemissi/fuzzming
cd fuzzming
cargo install --path .
```

---

## Quick start

Navigate to your Foundry project, then run:

```bash
fuzzming run
```

FuzzMing will prompt you for the target contract(s), model, and API key, then save your answers to `fuzzming.config` so you don't have to repeat them.

### Non-interactive (CI / scripted)

```bash
fuzzming run \
  --targets src/Vault.sol \
  --max-rounds 5 \
  --model openrouter/anthropic/claude-3.5-sonnet \
  --llm-key $OPENROUTER_KEY \
  --defaults
```

### Read everything from config

```bash
# First interactive run saves settings to fuzzming.config
fuzzming run

# All subsequent runs skip every prompt
fuzzming run --from-config
```

### Multiple contracts

```bash
fuzzming run --targets src/Vault.sol src/Token.sol src/Pool.sol --defaults
```

---

## Supported LLM providers

The `--model` prefix selects the provider. Pass the matching API key via `--llm-key` or `LLM_KEY`:

| Prefix | Provider | Example model |
|---|---|---|
| `openrouter/` | OpenRouter | `openrouter/anthropic/claude-3.5-sonnet` |
| `groq/` | Groq | `groq/llama-3.3-70b-versatile` |
| `openai/` | OpenAI | `openai/gpt-4o` |
| `anthropic/` | Anthropic | `anthropic/claude-3-5-sonnet-20241022` |

Sensitive values can be provided via environment variables to keep them out of shell history:

```bash
export LLM_MODEL=groq/llama-3.3-70b-versatile
export LLM_KEY=$GROQ_KEY
fuzzming run --targets src/Vault.sol --defaults
```

---

## fuzzming.config

On first run FuzzMing creates a `fuzzming.config` file in the current directory:

```
targets=src/Vault.sol
max_rounds=5
model=openrouter/anthropic/claude-3.5-sonnet
llm_key=sk-...
workspace_root=.
max_tokens=0
llm_timeout_secs=120
full_coverage_rounds=2
prompt_mode=guided
```

View it (API key masked):

```bash
fuzzming config
```

Delete it and re-prompt:

```bash
fuzzming config --reset
```

---

## Subcommands

| Command | Description |
|---|---|
| `fuzzming run` | Start a fuzzing session |
| `fuzzming guide` | Print the full CLI reference in the terminal |
| `fuzzming report` | Print a summary of the last run's artifacts |
| `fuzzming config` | View or reset the saved `fuzzming.config` |

### `fuzzming run` flags

| Flag | Default | Description |
|---|---|---|
| `--targets <PATHS...>` | — | Paths to target `.sol` files |
| `--max-rounds <N>` | 10 | Maximum fuzzing rounds per contract |
| `--model <ID>` | — | LLM model identifier (`LLM_MODEL` env var) |
| `--llm-key <KEY>` | — | API key for the model's provider (`LLM_KEY` env var) |
| `--workspace-root <DIR>` | `.` | Foundry project root |
| `--max-tokens <N>` | unlimited | Max tokens the LLM may generate per call |
| `--llm-timeout-secs <N>` | 120 | Per-call LLM timeout in seconds |
| `--full-coverage-rounds <N>` | 2 | Consecutive 100%-coverage rounds before stopping |
| `--defaults` | false | Skip all prompts; use flags and env vars |
| `--from-config` | false | Skip all prompts; read everything from `fuzzming.config` |
| `--interactive` | false | Force interactive prompts even when config exists |
| `--demo` | false | Mock run — full UI, no LLM calls, no tokens spent |
| `--verbose` | false | Enable verbose trace logs |

---

## How it works

Each fuzzing round follows this sequence:

```
1. Reader      — reads the target contract + previous-round artifacts
2. Security analysis (round 2+ only) — separate LLM pass that reviews fuzz output + confirmed bugs
3. Generator   — assembles a prompt, calls the LLM, parses the response
4. Executor    — writes Handler.sol + InvariantTest.sol; patches foundry.toml with
                 `test = "test/fuzzming"` so forge only sees generated tests
5. Fuzzer      — runs `forge build` (fast compile check), then `forge test`
                 both scoped to `test/fuzzming/` via the profile's `test` key
6. Orchestrator — accumulates bugs (one entry per unique invariant name), strips confirmed invariants, checks termination
7. Reporter    — emits a formatted findings summary when a contract's session ends
```

The session ends on **full coverage or round exhaustion** — not on the first bug. When an invariant breaks, FuzzMing records it, removes it from the next round's test, and keeps hunting for more bugs.

### Round outcomes

| Outcome | Action |
|---|---|
| Bug confirmed | Record bug, strip broken invariant, continue |
| Compile error | Feed compiler output to LLM, retry next round |
| Developer test failed | Feed error to LLM, retry next round |
| Full coverage reached | Stop — no more gaps to cover |
| Round budget exhausted | Report all bugs found across all rounds |

---

## Limitations

FuzzMing finds bugs by generating thousands of random call sequences and checking that properties hold after every step. This approach has known blind spots — classes of bug that invariant fuzzing structurally cannot detect regardless of how many rounds run.

### 1. Bugs with no observable behavioral difference

If a bug changes which internal code path executes but always produces the same output, no invariant can fail. There is no state where the buggy version and the correct version disagree on a return value or storage change.

**Example:** A redundant pre-check that uses the wrong variable — but a try/catch immediately below it handles every failure case anyway. Both paths return `0`. FuzzMing cannot write a rule that fails here because the contract's visible behavior is identical with or without the bug. Detecting this requires static analysis: a tool that reads code structure and flags "these two consecutive blocks always produce the same result."

### 2. Bugs in code that never executes during testing

Some code paths are gated on `tx.origin` — the original wallet that started a transaction. In Foundry invariant tests, `tx.origin` is always the test contract's own address, not a real user wallet. If the buggy code only runs when a specific registered address is `tx.origin`, the fuzzer will never trigger it: the test contract is never in the relevant mapping, so the condition is always false, and the code block is skipped on every single call.

FuzzMing handles this via Rule 21 and a dedicated `tx_origin_paths` analysis field: when `tx.origin` is detected in the source, the LLM is instructed to call the target from inside a handler using `vm.prank(addr, addr)` — the two-argument form sets both `msg.sender` and `tx.origin` — then store the result in a ghost variable for the invariant to check. This pattern successfully confirmed the discount-related bugs in the DynamicSwapFeeModule case study.

**Remaining risk:** contracts where the `tx.origin`-dependent path is never reached regardless of caller identity, or where the required state preconditions are too narrow for the fuzzer to stumble upon within the round budget.

### 3. Bugs that require chain-specific knowledge

FuzzMing reads the contract and uses its constants as-is. If a hardcoded constant is the wrong value for the chain the contract will actually be deployed on, FuzzMing has no way to know. That knowledge lives outside the contract entirely.

**Example:** A constant set to `2` with a comment saying "must equal the block time." The contract is internally consistent — `2` is used the same way everywhere. But the target chain produces a block every 0.45 seconds, not every 2 seconds, making the constant 4× too large. No amount of fuzzing the contract reveals this. The fix is a `--chain` flag that loads known parameters (block time, gas limits, oracle patterns) for the target chain, so the analysis stage can compare hardcoded constants against real-world values.

### 4. Bugs that require two adversarial actors

FuzzMing's invariant testing uses a single actor calling functions randomly. It has no model of one address deliberately trying to harm another. Attacks that require coordinated multi-transaction sequencing — where an attacker moves state before a victim's transaction to cause the victim to pay more or receive less — are invisible to a single-actor model regardless of how many rounds run.

**Example:** A fee formula that uses the live spot price instead of a time-averaged price. An attacker can execute a large swap to push the spot price far from the average, inflating the fee charged to any swap that follows in the same block. The attacker loses money — it is a pure griefing attack. Discovering it requires two actors: one that moves state adversarially, and one that checks whether the victim paid above a fair threshold. This is closer to game-theoretic simulation than property testing and would require a dedicated multi-actor adversarial mode.

### Summary

| Limitation | Status | What would catch it |
|---|---|---|
| Bug produces no observable difference | Open | Static analysis — code linter or formal verifier |
| `tx.origin`-gated code paths | Handled — Rule 21 + `vm.prank(addr, addr)` | Confirmed discount bugs in DynamicSwapFeeModule |
| Wrong constant for a specific chain | Open | `--chain` flag with known chain parameters |
| Attack requires two adversarial actors | Open | Multi-actor adversarial simulation mode |

These limitations are documented in detail in the [DynamicSwapFeeModule case study](docs/case-study-dynamicswapfeemodule.md), which compares FuzzMing against a professional audit on the same contract — 7 bugs found, 2 Shieldify findings confirmed, 5 missed by the audit.

---

## Logging

```bash
# Round-by-round progress
fuzzming run --verbose --targets src/Vault.sol ...

# Fine-grained tracing (via RUST_LOG)
RUST_LOG=debug fuzzming run --targets src/Vault.sol ...
```

---

## Contributing

FuzzMing is built on hexagonal architecture so that every language and fuzzer is a first-class citizen. Adding a new stack (Rust, Vyper, Move, Echidna, Medusa, cargo-fuzz) means writing new adapters: the orchestrator, session loop, LLM integration, and report format never change. That is the core design bet. The technical documentation for collaborators lives in [docs/](docs/):

| Document | What it covers |
|---|---|
| [docs/orchestrator.md](docs/orchestrator.md) | Session loop, termination logic, round coordination |
| [docs/generator.md](docs/generator.md) | 3-stage LLM call chain, prompt assembly, retry/repair |
| [docs/executor.md](docs/executor.md) | Write gateway — Solidity files, foundry.toml |
| [docs/fuzzer.md](docs/fuzzer.md) | Forge subprocess, output parsing, coverage |
| [docs/reader.md](docs/reader.md) | Read gateway — source files, coverage context |
| [docs/reporter.md](docs/reporter.md) | Report formatters and output adapters |
| [docs/shared.md](docs/shared.md) | Shared data layer — models, ports, requests, responses |
| [docs/entry.md](docs/entry.md) | CLI entry point — subcommands, flags, exit codes |
| [docs/composition.md](docs/composition.md) | Composition root — full wiring graph |
| [docs/case-study-dynamicswapfeemodule.md](docs/case-study-dynamicswapfeemodule.md) | FuzzMing vs. Shieldify audit — 7 bugs found in 23 min at $4.94, 2 Shieldify findings confirmed, 5 missed by the audit |

To add a new language or fuzzer, see the checklist in [docs/composition.md](docs/composition.md).

**How to contribute:**

1. Fork the repo and create a branch from `main`.
2. Read [docs/shared.md](docs/shared.md) first — understanding the shared data layer is the fastest way to orient yourself.
3. Keep changes inside one component if possible; cross-component changes must go through `src/shared/`.
4. Run `cargo test` before opening a PR — the fuzzer integration tests require Foundry to be installed.
5. Open a PR against `main` with a clear description of what changed and why.

---

## Contributors

Every contribution matters: code, docs, bug reports, ideas. Thank you to everyone who helps grow FuzzMing.

<div align="center">
  <table>
    <tr>
      <td align="center" style="padding: 16px;">
        <a href="https://github.com/AchrefHemissi">
          <img src="https://github.com/AchrefHemissi.png" width="96" style="border-radius: 50%;" /><br /><br />
          <b>AchrefHemissi</b>
        </a>
      </td>
      <td align="center" style="padding: 16px;">
        <a href="https://github.com/Dhia9030">
          <img src="https://github.com/Dhia9030.png" width="96" style="border-radius: 50%;" /><br /><br />
          <b>Dhia9030</b>
        </a>
      </td>
      <td align="center" style="padding: 16px;">
        <a href="https://github.com/HanineKhemir">
          <img src="https://github.com/HanineKhemir.png" width="96" style="border-radius: 50%;" /><br /><br />
          <b>HanineKhemir</b>
        </a>
      </td>
    </tr>
  </table>
</div>

---

## About this project

FuzzMing **started as** a **final year engineering project** by **[AchrefHemissi](https://github.com/AchrefHemissi)**, **[Dhia9030](https://github.com/Dhia9030)**, and **[HanineKhemir](https://github.com/HanineKhemir)**, students in computer engineering at **[INSAT — Institut National des Sciences Appliquées et de Technologie](https://insat.rnu.tn)**, with the support and guidance of **[Dar Blockchain](https://darblockchain.io)**.

We are grateful to everyone who guided us through this journey:

**Academic Supervisor**
- **Ms. Lilia Sfaxi** 

**Industry Mentors**
- **Mr. Nadhir Abdelatif**
- **Mr. Ayoub Amer**
- **Mr. Anas Hammou**

Their expertise, feedback, and encouragement made this project possible. Thank you ❤️

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

```
Copyright 2026 FuzzMing Contributors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
