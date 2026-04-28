use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportArtifacts {
    pub contract_name: String,
    pub fuzz_output: String,
    pub coverage_summary: Option<String>,
    pub call_sequences: Vec<String>,
    pub round_history: u32,
}
