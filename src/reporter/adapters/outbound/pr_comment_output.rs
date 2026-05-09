use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::reporter::ports::outbound::OutputPort;
use crate::shared::responses::stage_event::StageEvent;

pub struct PrCommentOutput {
    pub github_token: String,
    pub repo: String,
    pub pr_number: u64,
}

impl PrCommentOutput {
    pub fn new(github_token: String, repo: String, pr_number: u64) -> Self {
        Self { github_token, repo, pr_number }
    }
}

#[async_trait]
impl OutputPort for PrCommentOutput {
    async fn write(&self, output: &str) -> Result<()> {
        let url = format!(
            "https://api.github.com/repos/{}/issues/{}/comments",
            self.repo, self.pr_number
        );

        reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.github_token))
            .header("User-Agent", "fuzzming")
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "body": output }))
            .send()
            .await
            .context("failed to post GitHub PR comment")?
            .error_for_status()
            .context("GitHub API returned an error")?;

        Ok(())
    }

    async fn write_progress(&self, _output: &str) -> Result<()> {
        Ok(())
    }

    async fn handle_stage_event(&self, _event: StageEvent) -> Result<()> {
        Ok(())
    }
}
