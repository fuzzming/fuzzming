use anyhow::Result;
use fuzzming::{
    fuzzer::{
        adapters::outbound::{FileSystemFuzzerOutput, ForgeRunner},
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

fn vault_project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/foundry_vault")
}

fn signal(workspace: PathBuf) -> RoundSignal {
    RoundSignal {
        round: 1,
        config: SessionConfig {
            llm_url: String::new(),
            llm_key: String::new(),
            output_format: OutputFormat::Terminal,
            ci_mode: false,
            language: Language::Solidity,
            fuzzer: Fuzzer::Foundry,
            workspace_root: workspace,
        },
        contract_name: "Vault".to_string(),
        contract_path: "src/Vault.sol".to_string(),
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
    let use_case = RunFuzzerUseCase::new(
        Box::new(runner),
        Box::new(FileSystemFuzzerOutput::new(workspace.clone())),
    );

    let reports = use_case.run(vec![signal(workspace)]).await?;

    assert_eq!(reports.len(), 1);
    assert!(
        matches!(reports[0].outcome, FuzzOutcome::Pass),
        "expected Pass, got {:?}",
        reports[0].outcome
    );
    Ok(())
}

/// Forge test stdout is persisted to .fuzzming/Vault/fuzz_output.txt
#[tokio::test]
async fn fuzz_output_written_to_workspace() -> Result<()> {
    let workspace = vault_project();
    let runner = ForgeRunner::new(workspace.clone());
    let use_case = RunFuzzerUseCase::new(
        Box::new(runner),
        Box::new(FileSystemFuzzerOutput::new(workspace.clone())),
    );

    use_case.run(vec![signal(workspace.clone())]).await?;

    let output_path = workspace.join(".fuzzming/Vault/fuzz_output.txt");
    assert!(output_path.exists(), ".fuzzming/Vault/fuzz_output.txt not found");

    let content = std::fs::read_to_string(&output_path)?;
    assert!(
        content.contains("VaultInvariantTest"),
        "expected VaultInvariantTest in fuzz_output, got: {content}"
    );
    Ok(())
}
