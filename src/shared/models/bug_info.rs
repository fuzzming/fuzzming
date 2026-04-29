use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugInfo {
    pub invariant_name: String,
    pub call_sequence: String,
}
