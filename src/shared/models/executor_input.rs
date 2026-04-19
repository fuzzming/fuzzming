use crate::shared::models::{BodiesJson, FuzzerConfigArtifact};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorInput {
    pub bodies: BodiesJson,
    pub fuzzer_config: FuzzerConfigArtifact,
}
