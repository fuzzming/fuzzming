use serde::{Deserialize, Serialize};
use crate::interfaces::state::session_config::SessionConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub rounds_remaining: u32,
    pub current_round: u32,
    pub config: SessionConfig,
}
