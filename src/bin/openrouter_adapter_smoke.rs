use std::path::PathBuf;
use std::{env, fs};

use anyhow::{Context, Result};
use fuzzming::interfaces::artifacts::{AssembledPrompt, Message, Role};
use fuzzming::llm::adapters::openrouter_adapter::OpenRouterAdapter;
use fuzzming::llm::ports::{LlmGenerationPort, LlmGenerationRequest};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::from_env()?;

    let env_file =
        env::var("SMOKE_ENV_FILE").unwrap_or_else(|_| "test/llm_generation/.env.local".to_string());
    if PathBuf::from(&env_file).exists() {
        dotenvy::from_filename_override(&env_file)
            .with_context(|| format!("failed to load env file: {env_file}"))?;
    }

    let api_key = env::var("OPENROUTER_API_KEY").context("missing OPENROUTER_API_KEY")?;

    let source_code = match &args.source_path {
        Some(path) => fs::read_to_string(path)
            .with_context(|| format!("failed to read source file: {}", path.display()))?,
        None => default_source_code(),
    };

    let adapter = OpenRouterAdapter::new(api_key).with_default_model(args.model.clone());

    let prompt = AssembledPrompt {
        messages: vec![Message {
            role: Role::User,
            content: "Generate bodies and foundry config JSON for this contract. If round > 1, return minimal patch updates only.".to_string(),
        }],
        round: args.round,
        context_sections: vec![
            "Target output must match Rust structs BodiesJson and FoundryConfig.".to_string(),
            "For round 1 return full content. For later rounds prefer patch mode.".to_string(),
        ],
    };

    let request = LlmGenerationRequest {
        round: args.round,
        model: args.model,
        source_code,
        prompt,
        existing_bodies: None,
        existing_foundry_config: None,
    };

    let response = adapter.generate(request).await?;
    let pretty = serde_json::to_string_pretty(&response)?;

    let output_path = args.output_path.unwrap_or_else(|| {
        PathBuf::from(format!(
            "test/llm_generation/generated/openrouter_round_{}.json",
            args.round
        ))
    });

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory: {}", parent.display())
            })?;
        }
    }

    fs::write(&output_path, pretty)
        .with_context(|| format!("failed to write output json: {}", output_path.display()))?;

    println!("Saved generated output to {}", output_path.display());

    Ok(())
}

#[derive(Debug, Clone)]
struct Args {
    source_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    round: u32,
    model: String,
}

impl Args {
    fn from_env() -> Result<Self> {
        let mut source_path = env::var("SMOKE_SOURCE_PATH").ok().map(PathBuf::from);
        let mut output_path = env::var("SMOKE_OUTPUT_PATH").ok().map(PathBuf::from);
        let mut round = env::var("SMOKE_ROUND")
            .ok()
            .and_then(|x| x.parse::<u32>().ok())
            .unwrap_or(1);
        let mut model = env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "openrouter/openai/gpt-4o-mini".to_string());

        let mut it = env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--source" => {
                    let v = it.next().context("missing value for --source")?;
                    source_path = Some(PathBuf::from(v));
                }
                "--out" => {
                    let v = it.next().context("missing value for --out")?;
                    output_path = Some(PathBuf::from(v));
                }
                "--round" => {
                    let v = it.next().context("missing value for --round")?;
                    round = v.parse::<u32>().context("--round must be a u32")?;
                }
                "--model" => {
                    model = it.next().context("missing value for --model")?;
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => {
                    return Err(anyhow::anyhow!("unknown arg: {arg}. Use --help for usage."));
                }
            }
        }

        Ok(Self {
            source_path,
            output_path,
            round,
            model,
        })
    }
}

fn print_help() {
    println!("openrouter_adapter_smoke usage:");
    println!("  cargo run --bin openrouter_adapter_smoke -- --source test/llm_generation/contracts/SimpleVault.sol --out test/llm_generation/generated/result.json");
    println!("Flags:");
    println!("  --source <path>   Solidity contract source file path");
    println!("  --out <path>      Output JSON path (default: test/llm_generation/generated/openrouter_round_<round>.json)");
    println!("  --round <u32>     Round number (default: 1)");
    println!("  --model <name>    OpenRouter model (default: openrouter/openai/gpt-4o-mini)");
    println!("Env keys:");
    println!("  OPENROUTER_API_KEY (required)");
    println!("  OPENROUTER_MODEL, SMOKE_SOURCE_PATH, SMOKE_OUTPUT_PATH, SMOKE_ROUND, SMOKE_ENV_FILE (optional)");
}

fn default_source_code() -> String {
    r#"pragma solidity ^0.8.20;

contract SimpleVault {
    mapping(address => uint256) public balances;

    function deposit() external payable {
        balances[msg.sender] += msg.value;
    }

    function withdraw(uint256 amount) external {
        require(balances[msg.sender] >= amount, \"insufficient\");
        balances[msg.sender] -= amount;
        payable(msg.sender).transfer(amount);
    }
}
"#
    .to_string()
}
