use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "fuzzming",
    version,
    disable_help_flag = true,
    about = "AI-powered Solidity smart contract fuzzer"
)]
pub struct CliArgs {
    /// Print the full CLI reference and examples
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    pub help: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Flags for `fuzzming run`
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Paths to target Solidity contracts
    #[arg(short, long, num_args = 0..)]
    pub targets: Vec<String>,

    /// Maximum number of fuzzing rounds
    #[arg(short, long)]
    pub max_rounds: Option<u32>,

    /// LLM model identifier, e.g. groq/llama-3.3-70b-versatile
    #[arg(long, env = "LLM_MODEL")]
    pub model: Option<String>,

    /// LLM API Key
    #[arg(long, env = "LLM_KEY")]
    pub llm_key: Option<String>,

    /// Enable verbose logs
    #[arg(long, default_value_t = false)]
    pub verbose: bool,

    /// Foundry project root (defaults to current directory)
    #[arg(long)]
    pub workspace_root: Option<PathBuf>,

    /// Maximum tokens the LLM may generate per call (omit for no limit)
    #[arg(long)]
    pub max_tokens: Option<u32>,

    /// Force interactive prompts even when flags are provided
    #[arg(long, default_value_t = false)]
    pub interactive: bool,

    /// Skip all prompts and use default values (targets/model/key from flags or env vars)
    #[arg(long, default_value_t = false)]
    pub defaults: bool,

    /// Skip all prompts and read everything from fuzzming.config — fails if config is incomplete
    #[arg(long, default_value_t = false)]
    pub from_config: bool,

    /// Per-call LLM timeout in seconds (default: 120)
    #[arg(long, default_value_t = 120)]
    pub llm_timeout_secs: u64,

    /// Stop a contract after this many consecutive rounds with 100% coverage (default: 2)
    #[arg(long, default_value_t = 2)]
    pub full_coverage_rounds: u32,

    /// Run an interactive demo with mock adapters — no LLM calls, no tokens spent
    #[arg(long, default_value_t = false)]
    pub demo: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start a fuzzing session
    Run(RunArgs),

    /// Show the full CLI reference, all flags, subcommands, and usage examples
    Guide,

    /// Print a summary report from a previous fuzzming run
    Report {
        #[arg(long, value_name = "DIR")]
        workspace_root: Option<PathBuf>,
    },

    /// View or reset the saved fuzzming.config
    Config {
        /// Delete fuzzming.config — the next run will re-prompt for all settings
        #[arg(long, default_value_t = false)]
        reset: bool,
    },
}

pub fn parse_args() -> Result<CliArgs, clap::Error> {
    CliArgs::try_parse()
}
