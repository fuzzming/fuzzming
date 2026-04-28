use crate::shared::models::CoverageContext;
use crate::shared::models::SessionConfig;
use crate::shared::models::{BodiesJson, FoundryConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundSignal {
    pub round: u32,
    pub config: SessionConfig,
    /// Stem of the target contract file, e.g. "Vault" from "src/Vault.sol".
    /// Used to derive all per-contract paths; never comes from the LLM.
    pub contract_name: String,
    /// Path to the target contract relative to workspace_root, e.g. "src/Vault.sol".
    pub contract_path: String,
    pub source_code: String,
    pub fuzz_output: Option<String>,
    pub coverage_context: Option<CoverageContext>,
    pub existing_bodies: Option<BodiesJson>,
    pub existing_foundry_config: Option<FoundryConfig>,
}
