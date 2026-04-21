use serde::{Deserialize, Serialize};

use crate::shared::models::{BodiesJson, FoundryConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisStage {
    pub vulnerability_analysis: Vec<serde_json::Value>,
    pub handler_logic_pseudocode: serde_json::Value,
    pub invariant_mathematical_proofs: Vec<serde_json::Value>,
    pub critical_invariants: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodiesStage {
    pub bodies: BodiesJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigStage {
    pub foundry_config: FoundryConfig,
}
