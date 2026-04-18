use anyhow::Result;
use fuzzming::reader::adapters::foundry_fuzzer_reader::FoundryFuzzerReader;
use fuzzming::reader::ports::fuzzer_reader_port::FuzzerReaderPort;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the reader pointing to the directory where our json is
    let reader = FoundryFuzzerReader::new("examples/foundry_app".to_string());

    println!("Reading fuzzer output from examples/foundry_app/fuzz_output.json...\n");
    let result = reader.read_fuzzer_output("fuzz_output.json").await?;

    println!("=== Parsed Fuzzer Output ===");
    println!("{}", result);
    println!("============================");

    Ok(())
}
