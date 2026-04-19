use crate::shared::models::FoundryConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FuzzerConfigArtifact {
    Foundry(FoundryConfig),
    // Echidna(EchidnaConfig),  — future
    // Medusa(MedusaConfig),    — future
}
