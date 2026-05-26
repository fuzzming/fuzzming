use anyhow::Result;
use fuzzming::reader::{
    adapters::outbound::{FileSystemReader, SolidityContractReader},
    ports::inbound::ReaderRunPort,
    use_cases::read::ReadUseCase,
};
use fuzzming::shared::models::{CoverageContext, CoverageGap, GapType};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;

fn make_use_case(dir: &TempDir) -> ReadUseCase {
    let fs_reader = Arc::new(FileSystemReader::new(dir.path().to_path_buf()));
    let contract_reader = Arc::new(SolidityContractReader::new(Arc::clone(&fs_reader)));
    ReadUseCase::new(contract_reader, fs_reader)
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

#[tokio::test]
async fn get_coverage_context_returns_none_when_file_missing() -> Result<()> {
    let dir = TempDir::new()?;
    let use_case = make_use_case(&dir);

    let coverage = use_case.get_coverage_context("missing-coverage_context.json").await?;

    assert!(coverage.is_none());
    Ok(())
}

#[tokio::test]
async fn get_coverage_context_reads_enriched_json() -> Result<()> {
    let dir = TempDir::new()?;

    let context = CoverageContext {
        gaps: vec![
            CoverageGap {
                file: "src/Vault.sol".to_string(),
                line: 42,
                gap_type: GapType::Branch,
                source_context: vec!["41: if (x > 0) {".to_string(), "42:     revert();".to_string()],
            },
        ],
        line_found: 10,
        line_hit: 8,
        branch_found: 4,
        branch_hit: 3,
        function_found: 2,
        function_hit: 2,
    };

    fs::write(
        dir.path().join("coverage_context.json"),
        serde_json::to_string(&context)?,
    )
    .await?;

    let use_case = make_use_case(&dir);
    let loaded = use_case
        .get_coverage_context("coverage_context.json")
        .await?
        .expect("coverage should exist");

    assert_eq!(loaded.line_found, 10);
    assert_eq!(loaded.line_hit, 8);
    assert_eq!(loaded.gaps.len(), 1);
    assert_eq!(loaded.gaps[0].line, 42);
    assert!(matches!(loaded.gaps[0].gap_type, GapType::Branch));
    assert!(loaded.gaps[0].source_context[1].contains("revert"));
    Ok(())
}
