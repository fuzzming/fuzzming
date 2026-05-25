use crate::shared::models::SessionConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRequest {
    pub target_paths: Vec<String>,
    pub max_rounds: u32,
    pub config: SessionConfig,
}
