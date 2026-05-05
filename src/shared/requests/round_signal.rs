use serde::{Deserialize, Serialize};

use crate::shared::models::{BodiesJson, BugInfo, CoverageContext, FoundryConfig, SessionConfig};

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
    /// Bugs confirmed in previous rounds for this contract.
    /// The LLM uses these to avoid re-generating broken invariants;
    /// the executor strips them deterministically from Full responses.
    pub confirmed_bugs: Vec<BugInfo>,
}
