use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "fuzzming",
    version,
    about = "AI-powered Solidity smart contract fuzzer",
    long_about = "FuzzMing — AI-powered Solidity smart contract fuzzer.\n\
\n\
Point it at a Foundry project and it will:\n\
  • Analyse your contracts with an LLM\n\
  • Auto-generate invariant fuzz tests\n\
  • Run forge test and iterate until it finds bugs or coverage plateaus\n\
\n\
SUBCOMMANDS\n\
  guide                       Show the full CLI reference and examples\n\
  report [--workspace-root]   Print a summary of the last run (coverage, output)\n\
  config [--reset]            View or clear saved fuzzming.config\n\
\n\
Run \"fuzzming guide\" for detailed usage, flags, and examples."
)]
pub struct CliArgs {
    /// Optional subcommands (guide | report | config)
    #[command(subcommand)]
    pub command: Option<Command>,

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

    /// Maximum tokens the LLM may generate per call (default: 16384)
    #[arg(long, default_value_t = 16_384)]
    pub max_tokens: u32,

    /// Force interactive prompts even when flags are provided
    #[arg(long, default_value_t = false)]
    pub interactive: bool,

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
    /// Show the full CLI reference, all flags, subcommands, and usage examples
    ///
    /// Prints a structured guide to stdout — useful as a quick reference
    /// without leaving the terminal.  Equivalent to the online docs.
    Guide,

    /// Print a summary report from a previous fuzzming run
    ///
    /// Reads the .fuzzming/<Contract>/ artifact directories that were written
    /// during the last session and prints per-contract coverage percentages
    /// and the tail of each contract's forge fuzz output.
    ///
    /// Defaults to the current directory; use --workspace-root to point at a
    /// different Foundry project.
    Report {
        /// Path to the Foundry project that was fuzzed (defaults to ".")
        #[arg(
            long,
            value_name = "DIR",
            help = "Foundry project root to read .fuzzming/ artifacts from (default: \".\")]"
        )]
        workspace_root: Option<PathBuf>,
    },

    /// View or reset the saved fuzzming.config
    ///
    /// Without flags: prints every key in fuzzming.config.  The LLM API
    /// key is always masked (shown as ****) for security.
    ///
    /// With --reset: deletes fuzzming.config so the next `fuzzming` run
    /// walks you through the interactive setup again.
    Config {
        /// Delete fuzzming.config — the next run will re-prompt for all settings
        #[arg(long, default_value_t = false)]
        reset: bool,
    },
}

pub fn parse_args() -> CliArgs {
    CliArgs::parse()
}
