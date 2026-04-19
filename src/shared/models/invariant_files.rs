use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantFiles {
    pub invariant_file_path: String,
    pub foundry_toml_path: String,
    pub lcov_path: String,
    pub fuzz_output_path: String,
}
