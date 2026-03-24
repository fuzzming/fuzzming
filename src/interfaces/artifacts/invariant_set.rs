use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantSet {
    pub solidity: String,
    pub target_file_path: String,
}
