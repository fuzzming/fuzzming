use serde::{Deserialize, Serialize};
use crate::interfaces::artifacts::{BodiesJson, FoundryConfig};
use crate::interfaces::contexts::CoverageContext;
use crate::interfaces::state::SessionConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundSignal {
    pub round: u32,
    pub config: SessionConfig,
    pub source_code: String,
    pub fuzz_output: Option<String>,
    pub coverage_context: Option<CoverageContext>,
    pub existing_bodies: Option<BodiesJson>,
    pub existing_foundry_config: Option<FoundryConfig>,
}
