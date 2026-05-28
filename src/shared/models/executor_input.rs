use serde::{Deserialize, Serialize};

use crate::shared::models::{BodiesJson, FuzzerConfigArtifact, JsonBlockUpdate};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutorInput {
    /// Round 1: write everything from scratch.
    Full {
        bodies: BodiesJson,
        fuzzer_config: FuzzerConfigArtifact,
        source_pragma: String,
    },
    /// Round N: apply LLM-generated patch operations to the previous round's artifacts.
    Patch {
        existing_bodies: BodiesJson,
        bodies_updates: Vec<JsonBlockUpdate>,
        existing_config: FuzzerConfigArtifact,
        config_updates: Vec<JsonBlockUpdate>,
        source_pragma: String,
    },
}
