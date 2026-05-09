use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "fuzzming", about = "Solidity smart contract fuzzing assistant")]
pub struct CliArgs {
    /// Optional subcommands
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

    /// Run in CI mode (outputs structured for CI/CD pipelines)
    #[arg(long, default_value_t = false)]
    pub ci_mode: bool,

    /// Enable verbose logs
    #[arg(long, default_value_t = false)]
    pub verbose: bool,

    /// Foundry project root (defaults to current directory)
    #[arg(long)]
    pub workspace_root: Option<PathBuf>,

    /// Force interactive prompts even when flags are provided
    #[arg(long, default_value_t = false)]
    pub interactive: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show CLI guide and examples
    Guide,
}

pub fn parse_args() -> CliArgs {
    CliArgs::parse()
}
