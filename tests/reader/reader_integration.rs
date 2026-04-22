use anyhow::Result;
use fuzzming::reader::{
    adapters::outbound::{FileSystemReader, FoundryCoverageReader, SolidityContractReader},
    ports::inbound::ReaderRunPort,
    use_cases::read::ReadUseCase,
};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;

fn make_use_case(dir: &TempDir) -> ReadUseCase {
    let base = dir.path().to_str().unwrap().to_owned();
    let fs_reader = Arc::new(FileSystemReader::new(base.clone()));
    let contract_reader = Arc::new(SolidityContractReader::new(Arc::clone(&fs_reader)));
    let coverage_reader = Arc::new(FoundryCoverageReader::new(Arc::clone(&fs_reader)));
    ReadUseCase::new(contract_reader, coverage_reader, fs_reader)
}

#[tokio::test]
async fn get_contract_context_strips_comments() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(
        dir.path().join("Vault.sol"),
        "contract Vault {\n    // single line comment\n    /* block comment */\n    function deposit() external {} // inline comment\n}",
    )
    .await?;

    let use_case = make_use_case(&dir);
    let ctx = use_case.get_contract_context("Vault.sol", false).await?;

    assert!(!ctx.source_code.contains("single line comment"));
    assert!(!ctx.source_code.contains("block comment"));
    assert!(!ctx.source_code.contains("inline comment"));
    assert!(ctx.source_code.contains("function deposit"));
    Ok(())
}
