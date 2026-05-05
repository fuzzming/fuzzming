# Entry Points

The `entry/` module contains the two ways to invoke FuzzMing: interactively via the CLI and autonomously via a CI/CD pipeline. Both entry points read configuration from their environment, build a `SessionConfig` and `SessionRequest`, call `CompositionRoot::build`, and hand control to the orchestrator.

Neither entry point contains business logic — they translate external input into the internal data model and delegate immediately.

---

## Directory structure

```
src/entry/
├── cli/
│   ├── arg_parser.rs     # CliArgs — clap derive struct with all CLI flags
│   └── cli_runner.rs     # CliRunner — parses args, builds config, calls orchestrator, sets exit code
└── cicd/
    ├── cicd_adapter.rs   # CicdAdapter — reads env vars, builds config, calls orchestrator
    └── env_reader.rs     # CicdEnv — struct + read_cicd_env() for CI environment variables
```

---

## CLI entry point

### `src/main.rs`

The binary entry point. Initialises tracing, then delegates to `CliRunner`:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    CliRunner::new().run().await
}
```

Log level is controlled by the `RUST_LOG` environment variable:

```sh
RUST_LOG=info fuzzming --targets src/Vault.sol ...
```

Useful levels:
- `info` — round-by-round progress, forge start/finish, termination reason
- `debug` — everything above plus dependency internals

### `arg_parser.rs` — `CliArgs`

```rust
#[derive(Parser)]
pub struct CliArgs {
    /// Paths to target Solidity contracts (1 or more)
    #[arg(short, long, num_args = 1..)]
    pub targets: Vec<String>,

    /// Maximum number of fuzzing rounds (default: 10)
    #[arg(short, long, default_value_t = 10)]
    pub max_rounds: u32,

    /// LLM model identifier, e.g. openrouter/anthropic/claude-3.5-sonnet
    #[arg(long, env = "LLM_MODEL")]
    pub model: String,

    /// LLM API key for the model's provider
    #[arg(long, env = "LLM_KEY")]
    pub llm_key: String,

    /// Emit structured output for CI/CD (GitHub PR comment)
    #[arg(long, default_value_t = false)]
    pub ci_mode: bool,

    /// Foundry project root (default: current directory)
    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,
}
```

All sensitive values (`--model`, `--llm-key`) accept env vars so they never appear in shell history.

### `cli_runner.rs` — `CliRunner`

1. Parses `CliArgs`.
2. Builds `SessionConfig` from the args.
3. Builds `SessionRequest` from the args.
4. Calls `CompositionRoot::build(config)` → `Box<dyn OrchestratorPort>`.
5. Calls `orchestrator.run(request).await?`.
6. **Sets the process exit code:**

```rust
let has_bugs = matches!(outcome.reason, TerminationReason::Bug | TerminationReason::DevTestFailed)
    || !outcome.artifacts.call_sequences.is_empty();

if has_bugs {
    std::process::exit(1);
}
```

Exit code 0 means clean (pass, full coverage, or exhausted with no bugs found).
Exit code 1 means bugs were found or developer tests failed — CI pipelines treat this as a build failure.

The `Exhausted` case is covered by the `call_sequences` check: if any bugs were found across rounds before exhaustion, they are in `artifacts.call_sequences` and the exit code is 1.

---

## CI/CD entry point

### `cicd_adapter.rs` — `CicdAdapter`

Reads configuration from environment variables (the standard GitHub Actions model) instead of CLI flags. Uses the same `CompositionRoot::build` path as the CLI runner.

```rust
pub async fn run(&self) -> Result<()> {
    let env = read_cicd_env()?;
    let config = SessionConfig {
        model: env.model.clone(),
        llm_key: env.llm_key.clone(),
        output_format: OutputFormat::Ci,
        ...
    };
    let orchestrator = CompositionRoot::build(config);
    orchestrator.run(request).await?;
    Ok(())
}
```

In `OutputFormat::Ci` mode, `CompositionRoot` wires `PrCommentOutput` as the reporter output. The GitHub context (`GITHUB_TOKEN`, `GITHUB_REPOSITORY`, `PR_NUMBER`) is read from the environment inside `CompositionRoot::build` — `CicdAdapter` does not need to know about them.

### `env_reader.rs` — `CicdEnv`

```rust
pub struct CicdEnv {
    pub model: String,
    pub llm_key: String,
    pub target_paths: Vec<String>,
    pub max_rounds: u32,
    pub github_token: Option<String>,
    pub pr_number: Option<u64>,
    pub repo: Option<String>,
}
```

`read_cicd_env()` reads the standard env vars set by GitHub Actions. Currently a stub — pending implementation.

---

## Tracing

FuzzMing emits structured log events via `tracing`. The events logged at `info` level are:

| Event | When | Fields |
|---|---|---|
| `session started` | Before the loop | `contracts`, `max_rounds` |
| `round started` | Each round | `round`, `contracts` |
| `LLM started` | Before LLM call (per contract) | `contract`, `round` |
| `LLM done — executor writing files` | After LLM, before executor | `contract`, `round` |
| `stripped confirmed invariants` | When confirmed bugs are removed | `contract`, `stripped` |
| `executor done` | After executor | `contract`, `round` |
| `forge run started` | Before `forge test` | `round` |
| `forge run finished` | After `forge test` | `round` |
| `round complete — continuing` | Contract did not terminate | `contract`, `outcome`, `bugs_so_far`, `rounds_remaining` |
| `contract session terminated` | Contract terminated | `contract`, `reason`, `total_bugs`, `rounds` |

---

## Model identifier format

The `--model` value follows the litellm-rs prefix convention. The adapter derives the API key env var from the prefix:

| Prefix | Sets env var | Example model |
|---|---|---|
| `openrouter/` | `OPENROUTER_API_KEY` | `openrouter/anthropic/claude-3.5-sonnet` |
| `groq/` | `GROQ_API_KEY` | `groq/llama-3.3-70b-versatile` |
| `openai/` | `OPENAI_API_KEY` | `openai/gpt-4o` |
| `anthropic/` | `ANTHROPIC_API_KEY` | `anthropic/claude-3-5-sonnet-20241022` |

The `--llm-key` value is the key for whichever provider the prefix identifies.

---

## Example invocations

```sh
# Terminal mode — interactive development
fuzzming \
  --targets src/Vault.sol src/Token.sol \
  --max-rounds 5 \
  --model openrouter/anthropic/claude-3.5-sonnet \
  --llm-key $OPENROUTER_KEY \
  --workspace-root /path/to/foundry-project

# With env vars — no secrets in shell history
export LLM_MODEL=groq/llama-3.3-70b-versatile
export LLM_KEY=$GROQ_KEY
fuzzming --targets src/Vault.sol --max-rounds 10

# CI mode — output posted as GitHub PR comment
fuzzming \
  --targets src/Vault.sol \
  --ci-mode \
  --model openrouter/anthropic/claude-3.5-sonnet \
  --llm-key $OPENROUTER_KEY

# With debug logging
RUST_LOG=debug fuzzming --targets src/Vault.sol --model groq/... --llm-key $KEY
```
