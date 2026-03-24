use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::artifacts::RunnerResult;
use crate::fuzzer::ports::TestRunnerPort;

pub struct ForgeRunner {
    pub working_dir: String,
}

impl ForgeRunner {
    pub fn new(working_dir: String) -> Self {
        Self { working_dir }
    }
}

#[async_trait]
impl TestRunnerPort for ForgeRunner {
    async fn run_test(&self, profile_name: &str) -> Result<RunnerResult> {
        todo!()
    }

    async fn run_coverage(&self, profile_name: &str) -> Result<RunnerResult> {
        todo!()
    }
}
