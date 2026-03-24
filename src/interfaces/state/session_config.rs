use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Terminal,
    Ci,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub llm_url: String,
    pub llm_key: String,
    pub output_format: OutputFormat,
    pub ci_mode: bool,
}
