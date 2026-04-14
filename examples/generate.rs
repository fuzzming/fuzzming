/// Run with:  cargo run --example generate
///
/// Reads examples/Vault.bodies.json and writes the generated .sol files to
/// examples/output/test/handlers/VaultHandler.sol
/// examples/output/test/invariants/VaultInvariantTest.sol

use anyhow::Result;
use fuzzming::{
    executor::infrastructure::FileSystemWriter,
    executor::use_cases::write_bodies::write_bodies,
    executor::adapters::solidity_generator::SolidityGenerator,
    executor::ports::CodeGeneratorPort,
    interfaces::artifacts::BodiesJson,
};

#[tokio::main]
async fn main() -> Result<()> {
    let raw = std::fs::read_to_string("examples/Vault.bodies.json")?;
    let bodies: BodiesJson = serde_json::from_str(&raw)?;

    let writer = FileSystemWriter::new("examples/output".into());
    let generator = SolidityGenerator;

    write_bodies(&bodies, &writer).await?;
    println!("wrote  test/{}.bodies.json", bodies.meta.contract);

    generator.generate(&bodies, &writer).await?;
    println!("wrote  {}", bodies.handler.output_path);
    println!("wrote  {}", bodies.invariant_test.output_path);

    Ok(())
}
