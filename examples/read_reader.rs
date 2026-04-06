use fuzzming::reader::infrastructure::FileSystemReader;
use fuzzming::reader::reader::Reader;
use fuzzming::interfaces::contexts::InvariantFiles;

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

    // Read source code directly from FileSystemReader (without parser overhead)
    match fs.read_contract("examples/Simple.sol", true).await {
        Ok(source) => println!("{}", source),
        Err(e) => eprintln!("Failed to read contract: {}", e),
    }
}
