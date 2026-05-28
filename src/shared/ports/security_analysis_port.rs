use anyhow::Result;
use async_trait::async_trait;

use crate::shared::models::BugInfo;

pub struct SecurityAnalysisRequest {
    pub contract_name: String,
    pub source_code: String,
    pub confirmed_bugs: Vec<BugInfo>,
    pub fuzz_output: Option<String>,
    pub rounds_completed: u32,
    /// Security analysis produced by the previous round, if any.
    /// The LLM should refine and extend this rather than starting from scratch.
    pub previous_analysis: Option<String>,
}

#[async_trait]
pub trait SecurityAnalysisPort: Send + Sync {
    async fn analyze(&self, request: SecurityAnalysisRequest) -> Result<String>;
}
