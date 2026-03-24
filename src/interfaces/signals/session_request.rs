use serde::{Deserialize, Serialize};
use crate::interfaces::state::{SessionConfig, OutputFormat};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRequest {
    pub target_paths: Vec<String>,
    pub max_rounds: u32,
    pub config: SessionConfig,
    pub output_format: OutputFormat,
    pub ci_mode: bool,
}
