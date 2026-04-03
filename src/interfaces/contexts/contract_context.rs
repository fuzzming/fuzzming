use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractContext {
    pub functions: Vec<String>,
    pub state_variables: Vec<String>,
    pub modifiers: Vec<String>,
    pub constants: Vec<String>,
    pub contract_name: String,
    pub source_code: String, // full contract source (comments stripped if include_comments=false)
}
