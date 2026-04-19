use crate::shared::models::session_config::SessionConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub rounds_remaining: u32,
    pub current_round: u32,
    pub config: SessionConfig,
}
