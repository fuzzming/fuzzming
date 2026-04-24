use crate::reporter::ports::outbound::OutputPort;

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

impl OutputPort for PrCommentOutput {
    fn write(&self, _output: &str) {
        todo!()
    }
}
