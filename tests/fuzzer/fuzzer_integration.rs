use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Result;
use fuzzming::{
    fuzzer::{
        adapters::outbound::{FileSystemFuzzerOutput, ForgeRunner},
        ports::inbound::FuzzerRunPort,
        use_cases::RunFuzzerUseCase,
    },
    shared::{
        models::{Fuzzer, Language, PromptMode, SessionConfig},
        requests::round_signal::RoundSignal,
        responses::fuzz_report::FuzzOutcome,
    },
};

// Serialize tests that write to the fixture workspace to avoid filesystem races.
static WORKSPACE_MUT: Mutex<()> = Mutex::new(());

fn vault_project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/foundry_vault")
}

fn signal_for(contract_name: &str, workspace: PathBuf) -> RoundSignal {
    RoundSignal {
        round: 1,
        config: SessionConfig {
            model: String::new(),
            llm_key: String::new(),
            language: Language::Solidity,
            fuzzer: Fuzzer::Foundry,
            workspace_root: workspace,
            max_tokens: None,
            llm_timeout_secs: 120,
            full_coverage_rounds: 2,
            prompt_mode: PromptMode::Guided,
        },
        contract_name: contract_name.to_string(),
        contract_path: format!("src/{contract_name}.sol"),
        source_code: String::new(),
        fuzz_output: None,
        coverage_context: None,
        existing_bodies: None,
        existing_foundry_config: None,
        confirmed_bugs: vec![],
    }
}

fn signal(workspace: PathBuf) -> RoundSignal {
    signal_for("Vault", workspace)
}

fn broken_handler_source() -> &'static str {
    r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;
import {Test} from "forge-std/Test.sol";
contract BrokenContractHandler is Test {
    uint256 public ghost_value  // intentional compile error: missing semicolon
}
"#
}

fn broken_invariant_source() -> &'static str {
    r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;
import {Test} from "forge-std/Test.sol";
import {BrokenContractHandler} from "./BrokenContractHandler.sol";
contract BrokenContractInvariantTest is Test {
    BrokenContractHandler public handler;
    function setUp() external {
        handler = new BrokenContractHandler();
        targetContract(address(handler));
    }
    function invariant_always_true() external view {
        assert(true);
    }
}
"#
}

async fn create_broken_contract_dir(workspace: &PathBuf, name: &str) -> PathBuf {
    let dir = workspace.join("test/fuzzming").join(name);
    tokio::fs::create_dir_all(&dir).await.unwrap();
    tokio::fs::write(
        dir.join(format!("{name}Handler.sol")),
        broken_handler_source(),
    )
    .await
    .unwrap();
    tokio::fs::write(
        dir.join(format!("{name}InvariantTest.sol")),
        broken_invariant_source().replace("BrokenContract", name),
    )
    .await
    .unwrap();
    dir
}

/// Correct Vault — all invariants hold → Pass
#[tokio::test]
async fn correct_vault_invariants_pass() -> Result<()> {
    let _guard = WORKSPACE_MUT.lock().unwrap();
    let workspace = vault_project();
    let runner = ForgeRunner::new(workspace.clone());
    let use_case = RunFuzzerUseCase::new(
        Box::new(runner),
        Box::new(FileSystemFuzzerOutput::new(workspace.clone())),
        workspace.clone(),
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
    let _guard = WORKSPACE_MUT.lock().unwrap();
    let workspace = vault_project();
    let runner = ForgeRunner::new(workspace.clone());
    let use_case = RunFuzzerUseCase::new(
        Box::new(runner),
        Box::new(FileSystemFuzzerOutput::new(workspace.clone())),
        workspace.clone(),
    );

    use_case.run(vec![signal(workspace.clone())]).await?;

    let output_path = workspace.join(".fuzzming/Vault/fuzz_output.txt");
    assert!(
        output_path.exists(),
        ".fuzzming/Vault/fuzz_output.txt not found"
    );

    let content = std::fs::read_to_string(&output_path)?;
    assert!(
        content.contains("VaultInvariantTest"),
        "expected VaultInvariantTest in fuzz_output, got: {content}"
    );
    Ok(())
}

/// A contract whose test files have a compile error → CompileError outcome.
#[tokio::test]
async fn compile_error_gives_compile_error_outcome() -> Result<()> {
    let _guard = WORKSPACE_MUT.lock().unwrap();
    let workspace = vault_project();

    let broken_dir = create_broken_contract_dir(&workspace, "BrokenContract").await;

    let runner = ForgeRunner::new(workspace.clone());
    let use_case = RunFuzzerUseCase::new(
        Box::new(runner),
        Box::new(FileSystemFuzzerOutput::new(workspace.clone())),
        workspace.clone(),
    );

    let result = use_case
        .run(vec![signal_for("BrokenContract", workspace.clone())])
        .await;

    let _ = tokio::fs::remove_dir_all(&broken_dir).await;

    let reports = result?;
    assert_eq!(reports.len(), 1);
    assert!(
        matches!(reports[0].outcome, FuzzOutcome::CompileError),
        "expected CompileError, got {:?}",
        reports[0].outcome
    );
    Ok(())
}

/// When one contract has a compile error, the healthy peer still gets a Pass.
/// The erroring contract's test dir is stashed and restored automatically.
#[tokio::test]
async fn healthy_contract_runs_when_peer_has_compile_error() -> Result<()> {
    let _guard = WORKSPACE_MUT.lock().unwrap();
    let workspace = vault_project();

    let broken_dir = create_broken_contract_dir(&workspace, "BrokenContract").await;

    let runner = ForgeRunner::new(workspace.clone());
    let use_case = RunFuzzerUseCase::new(
        Box::new(runner),
        Box::new(FileSystemFuzzerOutput::new(workspace.clone())),
        workspace.clone(),
    );

    let signals = vec![
        signal_for("Vault", workspace.clone()),
        signal_for("BrokenContract", workspace.clone()),
    ];
    let result = use_case.run(signals).await;

    let _ = tokio::fs::remove_dir_all(&broken_dir).await;
    let _ = tokio::fs::remove_dir_all(workspace.join(".fuzzming-disabled/BrokenContract")).await;

    let reports = result?;
    assert_eq!(reports.len(), 2);

    let vault_report = reports.first().unwrap();
    let broken_report = reports.last().unwrap();

    assert!(
        matches!(vault_report.outcome, FuzzOutcome::Pass),
        "Vault should Pass, got {:?}",
        vault_report.outcome
    );
    assert!(
        matches!(broken_report.outcome, FuzzOutcome::CompileError),
        "BrokenContract should be CompileError, got {:?}",
        broken_report.outcome
    );
    Ok(())
}

/// A stash dir left from a previous crashed session is restored before fuzzing.
#[tokio::test]
async fn leftover_disabled_dirs_are_restored() -> Result<()> {
    let _guard = WORKSPACE_MUT.lock().unwrap();
    let workspace = vault_project();

    // Simulate a leftover disabled dir from a previous crash.
    let stash_dir = workspace.join(".fuzzming-disabled/LeftoverContract");
    let original_dir = workspace.join("test/fuzzming/LeftoverContract");

    let _ = tokio::fs::remove_dir_all(&stash_dir).await;
    let _ = tokio::fs::remove_dir_all(&original_dir).await;
    tokio::fs::create_dir_all(&stash_dir).await?;

    let runner = ForgeRunner::new(workspace.clone());
    let use_case = RunFuzzerUseCase::new(
        Box::new(runner),
        Box::new(FileSystemFuzzerOutput::new(workspace.clone())),
        workspace.clone(),
    );

    use_case.run(vec![signal(workspace.clone())]).await?;

    let was_restored = original_dir.exists();

    let _ = tokio::fs::remove_dir_all(&original_dir).await;
    let _ = tokio::fs::remove_dir_all(&stash_dir).await;

    assert!(
        was_restored,
        "expected .fuzzming-disabled/LeftoverContract to be restored to test/fuzzming/LeftoverContract"
    );
    Ok(())
}
