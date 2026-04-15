use std::path::PathBuf;
use std::{env, fs};

use anyhow::{Context, Result};
use fuzzming::llm::infrastructure::gateways::groq_adapter::GroqAdapter;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let args = Args::from_env()?;

    let api_key = env::var("GROQ_API_KEY")
        .or_else(|_| env::var("LLM_KEY"))
        .context("missing GROQ_API_KEY (or LLM_KEY)")?;

    let source_code = match &args.source_path {
        Some(path) => fs::read_to_string(&path)
            .with_context(|| format!("failed to read source file: {}", path.display()))?,
        None => default_source_code(),
    };

    let adapter = GroqAdapter::new(api_key).with_default_model(args.model.clone());

    let generated = adapter
        .generate_raw_files(&source_code, &args.model)
        .await?;

    let output_dir = resolve_output_dir(args.output_path, args.round);
    fs::create_dir_all(&output_dir).with_context(|| {
        format!(
            "failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    let handler_path = output_dir.join("Handler.sol");
    let invariants_path = output_dir.join("Invariants.t.sol");
    let config_path = output_dir.join("foundry.toml");

    fs::write(&handler_path, generated.handler_sol)
        .with_context(|| format!("failed to write handler file: {}", handler_path.display()))?;
    fs::write(&invariants_path, generated.invariants_sol).with_context(|| {
        format!(
            "failed to write invariant test file: {}",
            invariants_path.display()
        )
    })?;
    fs::write(&config_path, generated.foundry_toml)
        .with_context(|| format!("failed to write config file: {}", config_path.display()))?;

    println!("Saved generated files:");
    println!("- {}", handler_path.display());
    println!("- {}", invariants_path.display());
    println!("- {}", config_path.display());

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
        let mut model =
            env::var("GROQ_MODEL").unwrap_or_else(|_| "groq/llama-3.3-70b-versatile".to_string());

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
    println!("groq_adapter_smoke usage:");
    println!(
        "  cargo run --bin groq_adapter_smoke -- --source path/to/Contract.sol --out generated/"
    );
    println!("Flags:");
    println!("  --source <path>   Solidity contract source file path");
    println!("  --out <path>      Output directory or legacy file path (default: generated/)");
    println!("  --round <u32>     Round number (default: 1)");
    println!("  --model <name>    Groq model (default: groq/llama-3.3-70b-versatile)");
    println!("Env fallback:");
    println!("  GROQ_API_KEY or LLM_KEY (required; loaded from .env if present)");
    println!("  SMOKE_SOURCE_PATH, SMOKE_OUTPUT_PATH, SMOKE_ROUND, GROQ_MODEL");
}

fn resolve_output_dir(output_arg: Option<PathBuf>, round: u32) -> PathBuf {
    match output_arg {
        Some(path) => {
            if path.extension().is_some() {
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("generated"))
            } else {
                path
            }
        }
        None => {
            let _ = round;
            PathBuf::from("generated")
        }
    }
}

fn default_source_code() -> String {
    r#"pragma solidity ^0.8.20;

contract Vault {
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
