use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzReportContext {
    pub falsified_invariant: String,
    pub call_sequence: Vec<String>,
    pub exact_values: HashMap<String, String>,
}
