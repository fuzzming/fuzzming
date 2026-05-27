# Entry Points

The `entry/` module is the only place in the codebase that reads external input (CLI flags, environment variables, config file) and translates it into the internal data model. It builds a `SessionConfig` and `SessionRequest`, calls `CompositionRoot::build`, and hands control to the orchestrator.

Neither entry point contains business logic — they translate external input and delegate immediately.

---

## Directory structure

```
src/entry/
├── cli/
│   ├── arg_parser.rs     # CliArgs — clap derive struct; subcommands: run, guide, report, config
│   ├── cli_runner.rs     # CliRunner — dispatches subcommands, prints outcomes, sets exit code
│   ├── interactive.rs    # resolve_cli_config — merges fuzzming.config + flags + prompts
│   └── ui.rs             # CliUi — banner, info/error/success helpers
└── mod.rs
```

---

## `src/main.rs` — binary entry point

Initialises tracing, then delegates to `CliRunner`:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    CliRunner::new().run().await
}
```

Tracing is initialised inside `CliRunner::run` after parsing the `--verbose` flag. If `--verbose` is absent, `tracing` is set to `error` level only so the terminal output is clean.

---

## Subcommands

`CliRunner` dispatches on the parsed `Command` enum:

| Subcommand | Handler | Description |
|---|---|---|
| `run` | `handle_run(RunArgs)` | Build config, call orchestrator, print outcomes |
| `guide` | `print_extended_help()` | Print full CLI reference to stdout |
| `report` | `handle_report(workspace_root)` | Read `.fuzzming/` artifacts and print a summary |
| `config` | `handle_config(reset)` | View or delete `fuzzming.config` |

---

## `run` subcommand — `RunArgs`

```rust
pub struct RunArgs {
    pub targets: Vec<String>,          // Paths to target .sol files (0 or more)
    pub max_rounds: Option<u32>,        // Default: 10
    pub model: Option<String>,          // env: LLM_MODEL
    pub llm_key: Option<String>,        // env: LLM_KEY
    pub verbose: bool,                  // Enables RUST_LOG-level tracing
    pub workspace_root: Option<PathBuf>,// Default: current directory
    pub max_tokens: Option<u32>,        // None = no limit; 0 from config = no limit
    pub interactive: bool,              // Force prompts even when config exists
    pub defaults: bool,                 // Skip all prompts; use flags/env vars
    pub from_config: bool,              // Skip all prompts; read from fuzzming.config
    pub llm_timeout_secs: u64,          // Default: 120
    pub full_coverage_rounds: u32,      // Default: 2
    pub demo: bool,                     // Mock run — no LLM calls, no tokens
}
```

All sensitive values (`--model`, `--llm-key`) accept env vars so they never appear in shell history.

---

## Config resolution — `interactive.rs`

`resolve_cli_config(args)` merges three sources in priority order:

```
CLI flags  >  fuzzming.config  >  interactive prompts
```

1. Load `fuzzming.config` from the current directory (if present).
2. Overlay any explicit CLI flags (flags always win over saved config).
3. For any value still missing, prompt the user interactively — unless `--defaults` or `--from-config` is set.
4. Persist the resolved values back to `fuzzming.config` for next time.

`--from-config` fails immediately if any required value is absent from the config file.

`--defaults` fills missing values with defaults rather than prompting.

**`PromptMode` from config:**  
The `prompt_mode` key in `fuzzming.config` controls how verbose the LLM prompt rules are:

| Value | Rules | Best for |
|---|---|---|
| `concise` | 9 focused rules | Claude, GPT-4o, Gemini (capable models) |
| `guided` | 18 explicit rules | Open-source models that need more direction |

---

## `handle_run` — building config and calling the orchestrator

```rust
let config = SessionConfig {
    model:               resolved.model,
    llm_key:             resolved.llm_key,
    language:            Language::Solidity,
    fuzzer:              Fuzzer::Foundry,
    workspace_root:      resolved.workspace_root,
    max_tokens:          resolved.max_tokens,       // None when max_tokens == 0
    llm_timeout_secs:    resolved.llm_timeout_secs,
    full_coverage_rounds: resolved.full_coverage_rounds,
    prompt_mode:         resolved.prompt_mode,
};

let request = SessionRequest {
    target_paths: resolved.targets,
    max_rounds:   resolved.max_rounds,
    config:       config.clone(),
};

let orchestrator = CompositionRoot::build(config);
let outcomes = orchestrator.run(request).await?;
```

**Exit code:**

```rust
let has_bugs = outcomes.iter().any(|o| {
    matches!(o.reason, TerminationReason::Bug | TerminationReason::DevTestFailed | TerminationReason::CompileError)
        || !o.bugs.is_empty()
});

if has_bugs {
    std::process::exit(1);
}
```

Exit code 0 means clean (pass, full coverage, or exhausted with no bugs). Exit code 1 means bugs found or tests/compilation failed — CI pipelines treat this as a build failure.

---

## `report` subcommand

Reads `.fuzzming/<Contract>/outcome.json` and `lcov.info` for each contract in the workspace and prints a summary:

- Termination reason
- Rounds completed
- Bugs found (with call sequences)
- Line coverage percentage (from `lcov.info`, only shown on clean runs)

Coverage is not shown for runs that ended with `Bug`, `DevTestFailed`, or `CompileError` — the lcov file may be stale.

---

## `config` subcommand

| Flag | Effect |
|---|---|
| *(none)* | Print all keys from `fuzzming.config`; `llm_key` is always masked as `****` |
| `--reset` | Delete `fuzzming.config`; next `fuzzming run` re-prompts for all settings |

---

## Demo mode

`fuzzming run --demo` runs the full session loop with mock adapters — no LLM calls, no forge subprocesses, no tokens spent. It uses a temporary workspace and three scripted contracts (`TokenVault`, `StakingPool`, `PriceOracle`) with pre-canned outcomes (one bug found, one clean, one compile error).

Demo mode is the fastest way to see the UI and report format without any API key or Foundry installation.

---

## Tracing

FuzzMing emits structured log events via `tracing`. Without `--verbose`, only `error`-level events reach the terminal; the rich output comes from the `CliUi` helpers instead.

With `--verbose` the `EnvFilter` is built from `RUST_LOG`, which defaults to `info` if unset:

```bash
RUST_LOG=debug fuzzming run --verbose --targets src/Vault.sol ...
```

Key `info`-level events:

| Event | When | Fields |
|---|---|---|
| `session started` | Before the loop | `contracts`, `max_rounds` |
| `round started` | Each round | `round`, `contracts` |
| `LLM started` | Before LLM call (per contract) | `contract`, `round` |
| `LLM done — executor writing files` | After LLM, before executor | `contract`, `round` |
| `stripped confirmed invariants` | When confirmed bugs are removed | `contract`, `stripped` |
| `forge run started` | Before `forge test` | `round` |
| `forge run finished` | After `forge test` | `round` |
| `round complete — continuing` | Contract did not terminate | `contract`, `outcome`, `bugs_so_far`, `rounds_remaining` |
| `contract session terminated` | Contract terminated | `contract`, `reason`, `total_bugs`, `rounds` |

---

## Model identifier format

The `--model` value follows the litellm-rs prefix convention:

| Prefix | Provider | Example model |
|---|---|---|
| `openrouter/` | OpenRouter | `openrouter/anthropic/claude-3.5-sonnet` |
| `groq/` | Groq | `groq/llama-3.3-70b-versatile` |
| `openai/` | OpenAI | `openai/gpt-4o` |
| `anthropic/` | Anthropic | `anthropic/claude-3-5-sonnet-20241022` |

---

## Example invocations

```sh
# First run — interactive prompts, saves fuzzming.config
fuzzming run

# Subsequent runs — skip all prompts
fuzzming run --from-config

# Explicit flags, no prompts
fuzzming run \
  --targets src/Vault.sol src/Token.sol \
  --max-rounds 5 \
  --model openrouter/anthropic/claude-3.5-sonnet \
  --llm-key $OPENROUTER_KEY \
  --defaults

# With debug logging
RUST_LOG=debug fuzzming run --verbose --targets src/Vault.sol --model groq/... --llm-key $KEY

# Mock run — no LLM, no tokens
fuzzming run --demo

# View the full CLI reference
fuzzming guide

# Print a report from the last run
fuzzming report
fuzzming report --workspace-root ./my-project

# View saved config
fuzzming config
fuzzming config --reset
```
