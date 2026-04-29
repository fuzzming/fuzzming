use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReportArtifacts {
    pub fuzz_output: String,
    pub coverage_summary: Option<String>,
    pub call_sequences: Vec<String>,
}
