use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "fuzzming", about = "Solidity smart contract fuzzing assistant")]
pub struct CliArgs {
    /// Paths to target Solidity contracts
    #[arg(short, long, num_args = 1..)]
    pub targets: Vec<String>,

    /// Maximum number of fuzzing rounds
    #[arg(short, long, default_value_t = 10)]
    pub max_rounds: u32,

    /// LLM API URL
    #[arg(long, env = "LLM_URL")]
    pub llm_url: String,

    /// LLM API Key
    #[arg(long, env = "LLM_KEY")]
    pub llm_key: String,

    /// Run in CI mode (outputs structured for CI/CD pipelines)
    #[arg(long, default_value_t = false)]
    pub ci_mode: bool,

    /// Foundry project root (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub workspace_root: String,
}

pub fn parse_args() -> CliArgs {
    CliArgs::parse()
}
