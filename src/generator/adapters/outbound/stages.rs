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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigAdjustment {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchAnalysisStage {
    pub root_cause: String,
    pub affected_paths: Vec<String>,
    pub config_adjustments: Vec<ConfigAdjustment>,
    pub bodies_needed: Vec<String>,
    pub no_change_needed: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::PatchAnalysisStage;

    #[test]
    fn parses_patch_analysis_stage() {
        let payload = r#"{
            "rootCause": "ghost update order",
            "affectedPaths": ["handler.functions.deposit"],
            "configAdjustments": [
                {"path": "call_sequence_weights.withdraw", "reason": "stress window"}
            ],
            "bodiesNeeded": ["deposit"],
            "noChangeNeeded": ["withdraw"]
        }"#;

        let parsed: PatchAnalysisStage = serde_json::from_str(payload).expect("parse stage");
        assert_eq!(parsed.root_cause, "ghost update order");
        assert_eq!(parsed.affected_paths, vec!["handler.functions.deposit"]);
        assert_eq!(parsed.config_adjustments.len(), 1);
        assert_eq!(parsed.bodies_needed, vec!["deposit"]);
        assert_eq!(parsed.no_change_needed.unwrap(), vec!["withdraw"]);
    }
}
