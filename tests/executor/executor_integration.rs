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
    let writer = FileSystemWriter::new(output_dir());

    write_bodies(&bodies, &writer).await?;
    SolidityGenerator.generate(&bodies, &writer).await?;

    // bodies.json goes to .fuzzming/{Contract}/{Contract}.bodies.json
    let bodies_path = output_dir().join(".fuzzming/Vault/Vault.bodies.json");
    assert!(bodies_path.exists(), ".fuzzming/Vault/Vault.bodies.json was not created");
    let written: BodiesJson = serde_json::from_str(&std::fs::read_to_string(&bodies_path)?)?;
    assert_eq!(written.meta.contract, "Vault");

    // Solidity files go to test/fuzzming/{Contract}/ — isolated from user's test code
    let handler_src =
        std::fs::read_to_string(output_dir().join("test/fuzzming/Vault/VaultHandler.sol"))?;
    assert!(handler_src.contains(&format!("contract {} is Test {{", bodies.handler.contract_name)));
    for fn_name in bodies.handler.functions.keys() {
        assert!(handler_src.contains(&format!("function {fn_name}")));
    }

    let invariant_src =
        std::fs::read_to_string(output_dir().join("test/fuzzming/Vault/VaultInvariantTest.sol"))?;
    assert!(invariant_src.contains(&format!("contract {}", bodies.invariant_test.contract_name)));
    assert!(invariant_src.contains("function setUp"));
    let fn_count = invariant_src.matches("function ").count();
    assert_eq!(fn_count, bodies.invariant_test.invariants.len() + 1); // +1 for setUp

    println!("Output written to: {}", output_dir().display());

    Ok(())
}
