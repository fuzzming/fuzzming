use anyhow::Result;
use fuzzming::{
    fuzzer::{
        adapters::outbound::ForgeRunner,
        ports::inbound::FuzzerRunPort,
        use_cases::RunFuzzerUseCase,
    },
    shared::{
        models::{Fuzzer, Language, OutputFormat, SessionConfig},
        requests::round_signal::RoundSignal,
        responses::fuzz_report::FuzzOutcome,
    },
};
use std::path::PathBuf;

fn vault_project() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/foundry_vault")
        .to_str()
        .unwrap()
        .to_string()
}

fn signal(workspace: &str) -> RoundSignal {
    RoundSignal {
        round: 1,
        config: SessionConfig {
            llm_url: String::new(),
            llm_key: String::new(),
            output_format: OutputFormat::Terminal,
            ci_mode: false,
            language: Language::Solidity,
            fuzzer: Fuzzer::Foundry,
            workspace_root: workspace.to_string(),
        },
        source_code: String::new(),
        fuzz_output: None,
        coverage_context: None,
        existing_bodies: None,
        existing_foundry_config: None,
    }
}

/// Correct Vault — all invariants hold → Pass
#[tokio::test]
async fn correct_vault_invariants_pass() -> Result<()> {
    let workspace = vault_project();
    let runner = ForgeRunner::new(workspace.clone());
    let use_case = RunFuzzerUseCase::new(Box::new(runner));

    let report = use_case.run(signal(&workspace)).await?;

    assert!(
        matches!(report.outcome, FuzzOutcome::Pass),
        "expected Pass, got {:?}",
        report.outcome
    );
    Ok(())
}

/// Forge test stdout is persisted to .fuzzming/fuzz_output.txt inside the workspace
#[tokio::test]
async fn fuzz_output_written_to_workspace() -> Result<()> {
    let workspace = vault_project();
    let runner = ForgeRunner::new(workspace.clone());
    let use_case = RunFuzzerUseCase::new(Box::new(runner));

    use_case.run(signal(&workspace)).await?;

    let output_path = PathBuf::from(&workspace).join(".fuzzming/fuzz_output.txt");
    assert!(output_path.exists(), ".fuzzming/fuzz_output.txt not found");

    let content = std::fs::read_to_string(&output_path)?;
    assert!(
        content.contains("Suite result"),
        "unexpected fuzz_output content: {content}"
    );
    Ok(())
}
