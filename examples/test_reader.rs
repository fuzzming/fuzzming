use fuzzming::reader::adapters::solidity_contract_reader::SolidityContractReader;
use fuzzming::reader::ports::contract_reader_port::ContractReaderPort;

#[tokio::main]
async fn main() {
    let base_path = std::env::current_dir().unwrap().to_str().unwrap().to_string(); // points to fuzzming root
    let reader = SolidityContractReader::new(base_path);

    match reader.get_contract_context("examples/Simple.sol", false).await {
        Ok(source_code) => {
            println!("Successfully read contract source code!");
            println!("--- Source Code Without Imports ---");
            println!("{}", source_code);
            println!("-----------------------------------");
        }
        Err(e) => {
            eprintln!("Failed to read contract: {}", e);
        }
    }
}
