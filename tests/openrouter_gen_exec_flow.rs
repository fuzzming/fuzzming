use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::TempDir;

use fuzzming::executor::adapters::outbound::{
    FileSystemWriter, FoundryConfigWriter, SolidityGenerator,
};
use fuzzming::executor::ports::inbound::ExecutorRunPort;
use fuzzming::executor::use_cases::apply_patch::apply_patches;
use fuzzming::executor::use_cases::execute::ExecuteUseCase;
use fuzzming::generator::adapters::outbound::{LiteLlmClient, LiteLlmGenerationAdapter};
use fuzzming::generator::ports::inbound::GeneratorRunPort;
use fuzzming::generator::use_cases::run::GeneratorRunUseCase;
use fuzzming::reader::adapters::outbound::{
    FileSystemReader, FoundryCoverageReader, SolidityContractReader,
};
use fuzzming::reader::ports::inbound::ReaderRunPort;
use fuzzming::reader::use_cases::ReadUseCase;
use fuzzming::shared::models::{
    BodiesJson, FoundryConfig, Fuzzer, FuzzerConfigArtifact, Language, OutputFormat, SessionConfig,
};
use fuzzming::shared::requests::round_signal::RoundSignal;
use fuzzming::shared::responses::llm_signal::LlmStatus;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_project() -> PathBuf {
    repo_root().join("tests/fixtures/foundry_vault")
}

fn example_contract() -> PathBuf {
    repo_root().join("examples/Vault.sol")
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst).with_context(|| format!("create dir {}", dst.display()))?;
    }
    for entry in fs::read_dir(src).with_context(|| format!("read dir {}", src.display()))? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)
                .with_context(|| format!("copy {}", src_path.display()))?;
        }
    }
    Ok(())
}

fn print_generated(workspace_root: &str, bodies: &BodiesJson, round: u32) -> Result<()> {
    let handler_path = Path::new(workspace_root).join(&bodies.handler.output_path);
    let invariant_path = Path::new(workspace_root).join(&bodies.invariant_test.output_path);

    let handler = fs::read_to_string(&handler_path)
        .with_context(|| format!("read handler {}", handler_path.display()))?;
    let invariants = fs::read_to_string(&invariant_path)
        .with_context(|| format!("read invariants {}", invariant_path.display()))?;

    println!(
        "round {round} handler ({}):\n{}",
        bodies.handler.output_path, handler
    );
    println!(
        "round {round} invariants ({}):\n{}",
        bodies.invariant_test.output_path, invariants
    );
    Ok(())
}

fn persist_generated(
    repo_root: &Path,
    workspace_root: &str,
    bodies: &BodiesJson,
    round: u32,
) -> Result<()> {
    let out_dir = repo_root.join(format!("tests/output/gen_exec_round_{round}"));
    let handler_src = Path::new(workspace_root).join(&bodies.handler.output_path);
    let invariant_src = Path::new(workspace_root).join(&bodies.invariant_test.output_path);
    let foundry_src = Path::new(workspace_root).join("foundry.toml");
    let bodies_src =
        Path::new(workspace_root).join(format!("test/{}.bodies.json", bodies.meta.contract));

    let handler_dst = out_dir.join(&bodies.handler.output_path);
    let invariant_dst = out_dir.join(&bodies.invariant_test.output_path);
    let foundry_dst = out_dir.join("foundry.toml");
    let bodies_dst = out_dir.join(format!("test/{}.bodies.json", bodies.meta.contract));

    if let Some(parent) = handler_dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = invariant_dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = bodies_dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&handler_src, &handler_dst)
        .with_context(|| format!("copy {}", handler_src.display()))?;
    fs::copy(&invariant_src, &invariant_dst)
        .with_context(|| format!("copy {}", invariant_src.display()))?;
    if foundry_src.exists() {
        if let Some(parent) = foundry_dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&foundry_src, &foundry_dst)
            .with_context(|| format!("copy {}", foundry_src.display()))?;
    }
    if bodies_src.exists() {
        fs::copy(&bodies_src, &bodies_dst)
            .with_context(|| format!("copy {}", bodies_src.display()))?;
    }
    Ok(())
}

#[tokio::test]
#[ignore]
async fn openrouter_generator_executor_three_rounds() -> Result<()> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY is required for this test")?;
    let model = std::env::var("OPENROUTER_MODEL")
        .unwrap_or_else(|_| "openrouter/anthropic/claude-haiku-4.5".to_string());

    let temp = TempDir::new().context("create temp workspace")?;
    copy_dir_all(&fixture_project(), temp.path())?;

    let example_source = example_contract();
    let target_source = temp.path().join("src/Vault.sol");
    fs::copy(&example_source, &target_source)
        .with_context(|| format!("copy example contract to {}", target_source.display()))?;

    let workspace_root = temp.path().to_string_lossy().to_string();

    let fs_reader = Arc::new(FileSystemReader::new(workspace_root.clone()));
    let contract_reader = Arc::new(SolidityContractReader::new(fs_reader.clone()));
    let coverage_reader = Arc::new(FoundryCoverageReader::new(fs_reader.clone()));
    let reader = ReadUseCase::new(contract_reader, coverage_reader, fs_reader.clone());

    let source_code = reader
        .get_contract_context("src/Vault.sol", false)
        .await
        .context("read contract source")?
        .source_code;

    let client = Box::new(LiteLlmClient::new(model.clone(), Some(0.2), Some(4096)));
    let gateway = Box::new(LiteLlmGenerationAdapter::new(model, api_key, client));
    let generator = GeneratorRunUseCase::new(gateway);

    let writer = FileSystemWriter::new(workspace_root.clone());
    let code_gen = Arc::new(SolidityGenerator);
    let config_writer = Arc::new(FoundryConfigWriter);
    let executor = ExecuteUseCase::new(writer, code_gen, config_writer);

    let config = SessionConfig {
        llm_url: "openrouter".to_string(),
        llm_key: "redacted".to_string(),
        output_format: OutputFormat::Terminal,
        ci_mode: false,
        language: Language::Solidity,
        fuzzer: Fuzzer::Foundry,
        workspace_root: workspace_root.clone(),
    };

    let mut current_bodies: Option<BodiesJson> = None;
    let mut current_config: Option<FoundryConfig> = None;

    for round in 1..=3 {
        let signal = RoundSignal {
            round,
            config: config.clone(),
            source_code: source_code.clone(),
            fuzz_output: None,
            coverage_context: None,
            existing_bodies: current_bodies.clone(),
            existing_foundry_config: current_config.clone(),
        };

        let llm_signal = generator.run(signal).await?;
        if !matches!(llm_signal.status, LlmStatus::Done) {
            bail!("LLM failed in round {round}: {:?}", llm_signal.reason);
        }

        let result = llm_signal.result.context("missing LLM result")?;
        let usage = &result.usage;
        println!(
            "round {round} usage: calls={}, prompt={}, completion={}, total={}, cached={}, reasoning={}, thinking={}",
            usage.calls,
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens,
            usage.cached_prompt_tokens,
            usage.reasoning_tokens,
            usage.thinking_tokens
        );

        let (next_bodies, next_config) = match result.response {
            fuzzming::generator::domain::generation_response::GenerationResponse::Full {
                bodies,
                foundry_config,
            } => {
                executor
                    .execute(fuzzming::shared::models::ExecutorInput::Full {
                        bodies: bodies.clone(),
                        fuzzer_config: FuzzerConfigArtifact::Foundry(foundry_config.clone()),
                    })
                    .await
                    .context("executor full")?;
                print_generated(&workspace_root, &bodies, round)?;
                persist_generated(&repo_root(), &workspace_root, &bodies, round)?;
                (bodies, foundry_config)
            }
            fuzzming::generator::domain::generation_response::GenerationResponse::Patch {
                bodies_updates,
                foundry_config_updates,
            } => {
                let existing_bodies = current_bodies.clone().context("missing bodies")?;
                let existing_config = current_config.clone().context("missing config")?;

                executor
                    .execute(fuzzming::shared::models::ExecutorInput::Patch {
                        existing_bodies: existing_bodies.clone(),
                        bodies_updates: bodies_updates.clone(),
                        existing_config: FuzzerConfigArtifact::Foundry(existing_config.clone()),
                        config_updates: foundry_config_updates.clone(),
                    })
                    .await
                    .context("executor patch")?;

                let patched_bodies = apply_patches(existing_bodies, &bodies_updates)?;
                let patched_artifact = apply_patches(
                    FuzzerConfigArtifact::Foundry(existing_config),
                    &foundry_config_updates,
                )?;
                let patched_config = match patched_artifact {
                    FuzzerConfigArtifact::Foundry(config) => config,
                };
                print_generated(&workspace_root, &patched_bodies, round)?;
                persist_generated(&repo_root(), &workspace_root, &patched_bodies, round)?;
                (patched_bodies, patched_config)
            }
        };

        current_bodies = Some(next_bodies);
        current_config = Some(next_config);
    }

    Ok(())
}
