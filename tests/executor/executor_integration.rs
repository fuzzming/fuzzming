use anyhow::Result;
use fuzzming::{
    executor::{
        adapters::outbound::{FileSystemWriter, SolidityGenerator},
        ports::outbound::CodeGeneratorPort,
        use_cases::write_bodies::write_bodies,
    },
    shared::models::BodiesJson,
};
use std::path::PathBuf;

fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/output")
}

#[tokio::test]
async fn executor_generates_vault_files() -> Result<()> {
    let bodies: BodiesJson = serde_json::from_str(include_str!("../fixtures/Vault.bodies.json"))?;
    let writer = FileSystemWriter::new(output_dir().to_str().unwrap().to_owned());

    write_bodies(&bodies, &writer).await?;
    SolidityGenerator.generate(&bodies, &writer).await?;

    // --- assertions ---

    let bodies_path = output_dir().join("test/Vault.bodies.json");
    assert!(
        bodies_path.exists(),
        "test/Vault.bodies.json was not created"
    );
    let written: BodiesJson = serde_json::from_str(&std::fs::read_to_string(&bodies_path)?)?;
    assert_eq!(written.meta.contract, "Vault");

    let handler_src = std::fs::read_to_string(output_dir().join("test/handlers/VaultHandler.sol"))?;
    assert!(handler_src.contains("pragma solidity ^0.8.20"));
    assert!(handler_src.contains("contract VaultHandler is BaseHandler"));
    assert!(handler_src.contains("function handler_deposit"));
    assert!(handler_src.contains("function handler_withdraw"));
    assert!(handler_src.contains("ghost_totalDeposits"));

    let invariant_src =
        std::fs::read_to_string(output_dir().join("test/invariants/VaultInvariantTest.sol"))?;
    assert!(invariant_src.contains("contract VaultInvariantTest is Test"));
    assert!(invariant_src.contains("function setUp"));
    assert!(invariant_src.contains("invariant_totalAssetsMatch"));
    assert!(invariant_src.contains("invariant_depositCapRespected"));

    println!("Output written to: {}", output_dir().display());

    Ok(())
}
