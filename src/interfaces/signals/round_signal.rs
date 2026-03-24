use serde::{Deserialize, Serialize};
use crate::interfaces::state::SessionConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundSignal {
    pub round: u32,
    pub config: SessionConfig,
}
