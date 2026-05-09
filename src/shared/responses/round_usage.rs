use serde::{Deserialize, Serialize};

use crate::shared::models::GenerationUsage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundUsage {
    pub contract_name: String,
    pub round: u32,
    pub usage: GenerationUsage,
}
