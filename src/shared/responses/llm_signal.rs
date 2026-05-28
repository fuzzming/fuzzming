use crate::generator::domain::generation_response::GenerationResult;
use crate::shared::models::BodiesJson;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmStatus {
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSignal {
    pub status: LlmStatus,
    pub result: Option<GenerationResult>,
    pub reason: Option<String>,
    /// The final merged bodies after stripping and patch application.
    /// Used to look up invariant code when a bug is first confirmed by forge.
    #[serde(default)]
    pub final_bodies: Option<BodiesJson>,
    /// Invariant code bodies recovered at strip time, keyed by invariant name.
    /// Used to backfill BugInfo.invariant_code in the session state.
    #[serde(default)]
    pub stripped_invariant_codes: std::collections::HashMap<String, String>,
}
