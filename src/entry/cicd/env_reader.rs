use anyhow::Result;

#[derive(Debug, Clone)]
pub struct CicdEnv {
    pub model: String,
    pub llm_key: String,
    pub target_paths: Vec<String>,
    pub max_rounds: u32,
    pub github_token: Option<String>,
    pub pr_number: Option<u64>,
    pub repo: Option<String>,
}

pub fn read_cicd_env() -> Result<CicdEnv> {
    todo!()
}
