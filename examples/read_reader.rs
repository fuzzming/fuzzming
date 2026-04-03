use fuzzming::reader::infrastructure::FileSystemReader;
use fuzzming::reader::reader::Reader;
use fuzzming::interfaces::contexts::InvariantFiles;
use fuzzming::llm::ports::LlmReaderPort; // bring trait into scope so methods on Reader are available

#[tokio::main]
async fn main() {
    let base = std::env::current_dir().unwrap().to_str().unwrap().to_string();
    let fs = FileSystemReader::new(base.clone());

    let inv = InvariantFiles {
        invariant_file_path: "examples/invariants.json".to_string(),
        foundry_toml_path: "foundry.toml".to_string(),
        lcov_path: "examples/lcov.info".to_string(),
        fuzz_output_path: "examples/fuzz_output.txt".to_string(),
    };

    let reader = Reader::new(fs, inv);

    // Contract context - print raw source code without pragma/imports
    match reader.get_contract_context("examples/Simple.sol", true).await {
        Ok(ctx) => println!("{}", ctx.source_code),
        Err(e) => eprintln!("Failed to read contract: {}", e),
    }
}
