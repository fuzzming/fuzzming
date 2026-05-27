use crate::shared::models::FoundryConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FuzzerConfigArtifact {
    Foundry(FoundryConfig),
    // Echidna(EchidnaConfig),  // Reserved for future support.
    // Medusa(MedusaConfig),    // Reserved for future support.
}
