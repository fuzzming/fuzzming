use anyhow::Result;
use fuzzming::reader::{
    adapters::outbound::{FileSystemReader, FoundryCoverageReader, SolidityContractReader},
    ports::inbound::ReaderRunPort,
    use_cases::read::ReadUseCase,
};
use fuzzming::shared::models::GapType;
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

#[tokio::test]
async fn get_coverage_context_returns_none_when_lcov_missing() -> Result<()> {
    let dir = TempDir::new()?;
    let use_case = make_use_case(&dir);

    let coverage = use_case.get_coverage_context("missing-lcov.info").await?;

    assert!(coverage.is_none());
    Ok(())
}

#[tokio::test]
async fn get_coverage_context_parses_and_enriches_gaps() -> Result<()> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("src")).await?;

    let contract = [
        "contract Vault {",
        "    uint256 x;",
        "",
        "    function setUp() public {}",
        "    function helper() internal {}",
        "    function invariant_totalAssets() public view {}",
        "    function another() public {}",
        "    function branchy(uint256 v) public {",
        "        if (v > 0) {",
        "            x = v;",
        "        }",
        "    }",
        "}",
    ]
    .join("\n");
    fs::write(dir.path().join("src/Vault.sol"), contract).await?;

    let lcov = [
        "SF:src/Vault.sol",
        "FN:6,invariant_totalAssets",
        "FNDA:0,invariant_totalAssets",
        "DA:7,0",
        "BRDA:9,0,0,-",
        "LF:2",
        "LH:0",
        "BRF:1",
        "BRH:0",
        "FNF:1",
        "FNH:0",
        "end_of_record",
    ]
    .join("\n");
    fs::write(dir.path().join("lcov.info"), lcov).await?;

    let use_case = make_use_case(&dir);
    let coverage = use_case
        .get_coverage_context("lcov.info")
        .await?
        .expect("coverage should exist");

    assert_eq!(coverage.line_found, 2);
    assert_eq!(coverage.line_hit, 0);
    assert_eq!(coverage.branch_found, 1);
    assert_eq!(coverage.branch_hit, 0);
    assert_eq!(coverage.function_found, 1);
    assert_eq!(coverage.function_hit, 0);

    let fn_gap = coverage
        .gaps
        .iter()
        .find(|g| {
            matches!(g.gap_type, GapType::Function) && g.file == "src/Vault.sol" && g.line == 6
        })
        .expect("expected function gap");
    assert!(!fn_gap.source_context.is_empty());
    assert!(fn_gap
        .source_context
        .iter()
        .any(|l| l.contains("6:     function invariant_totalAssets() public view {}")));

    let line_gap = coverage
        .gaps
        .iter()
        .find(|g| matches!(g.gap_type, GapType::Line) && g.file == "src/Vault.sol" && g.line == 7)
        .expect("expected line gap");
    assert!(line_gap.source_context.iter().any(|l| l.starts_with("7: ")));

    let branch_gap = coverage
        .gaps
        .iter()
        .find(|g| matches!(g.gap_type, GapType::Branch) && g.file == "src/Vault.sol" && g.line == 9)
        .expect("expected branch gap");
    assert!(branch_gap
        .source_context
        .iter()
        .any(|l| l.starts_with("9: ")));

    Ok(())
}

#[tokio::test]
async fn get_coverage_context_supports_absolute_sf_paths() -> Result<()> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("src")).await?;

    let abs_file = dir.path().join("src/Absolute.sol");
    let contract = ["contract Absolute {", "    uint256 x;", "}"].join("\n");
    fs::write(&abs_file, contract).await?;

    let abs_path = abs_file
        .to_str()
        .expect("absolute path should be valid UTF-8")
        .to_string();
    let lcov = [
        format!("SF:{abs_path}"),
        "DA:2,0".to_string(),
        "LF:1".to_string(),
        "LH:0".to_string(),
        "BRF:0".to_string(),
        "BRH:0".to_string(),
        "FNF:0".to_string(),
        "FNH:0".to_string(),
        "end_of_record".to_string(),
    ]
    .join("\n");
    fs::write(dir.path().join("absolute-lcov.info"), lcov).await?;

    let use_case = make_use_case(&dir);
    let coverage = use_case
        .get_coverage_context("absolute-lcov.info")
        .await?
        .expect("coverage should exist");

    let gap = coverage
        .gaps
        .iter()
        .find(|g| matches!(g.gap_type, GapType::Line) && g.line == 2)
        .expect("expected line gap");
    assert_eq!(gap.file, abs_path);
    assert!(gap
        .source_context
        .iter()
        .any(|l| l.contains("2:     uint256 x;")));

    Ok(())
}
