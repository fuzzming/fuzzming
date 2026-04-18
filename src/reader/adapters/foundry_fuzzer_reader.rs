use crate::reader::ports::fuzzer_reader_port::FuzzerReaderPort;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

pub struct FoundryFuzzerReader {
    base_path: PathBuf,
}

impl FoundryFuzzerReader {
    pub fn new(base_path: String) -> Self {
        Self {
            base_path: PathBuf::from(base_path),
        }
    }

    async fn read_json(&self, path: &str) -> Result<String> {
        let full_path = self.base_path.join(path);
        fs::read_to_string(&full_path)
            .await
            .context(format!("Failed to read fuzzer output JSON file at {:?}", full_path))
    }
}

// We map to a HashMap since the root JSON object uses contract names as keys.
type FoundryFuzzOutput = HashMap<String, ContractTestSuite>;

#[derive(Deserialize, Debug)]
struct ContractTestSuite {
    #[serde(default)]
    test_results: HashMap<String, TestResult>,
}

#[derive(Deserialize, Debug)]
struct TestResult {
    pub status: String,
    pub reason: Option<String>,
    // We parse inner json structs as Values to safely ignore deep unneeded fields 
    // and easily format the ones we keep.
    pub counterexample: Option<Value>,
}

#[async_trait]
impl FuzzerReaderPort for FoundryFuzzerReader {
    async fn read_fuzzer_output(&self, path: &str) -> Result<String> {
        let raw_json = self.read_json(path).await?;
        let parsed_output: FoundryFuzzOutput = serde_json::from_str(&raw_json)
            .context("Failed to parse the Forge JSON fuzz output structure")?;

        let mut failed_summary = String::new();
        let mut failed_count = 0;

        for (contract_name, suite) in &parsed_output {
            let mut contract_has_failures = false;
            let mut contract_report = String::new();

            for (test_name, result) in &suite.test_results {
                if result.status == "Failure" {
                    contract_has_failures = true;
                    failed_count += 1;

                    contract_report.push_str(&format!("  Test: {}\n", test_name));
                    
                    if let Some(reason) = &result.reason {
                        contract_report.push_str(&format!("    Reason: {}\n", reason));
                    }
                    
                    if let Some(ce) = &result.counterexample {
                        // Pretty print the counterexample sequence/args using serde json
                        // This natively exposes caller, args, and trace strings to the LLM
                        let ce_json = serde_json::to_string_pretty(ce).unwrap_or_default();
                        contract_report.push_str(&format!("    Counterexample: {}\n", ce_json));
                    }
                    
                    contract_report.push_str("\n");
                }
            }

            if contract_has_failures {
                failed_summary.push_str(&format!("Contract: {}\n", contract_name));
                failed_summary.push_str(&contract_report);
            }
        }

        if failed_count == 0 {
            return Ok("All fuzzer tests passed successfully.".to_string());
        }

        Ok(format!(
            "Found {} failing tests:\n\n{}",
            failed_count, failed_summary
        ))
    }
}
