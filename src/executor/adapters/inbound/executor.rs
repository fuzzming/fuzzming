use anyhow::Result;
use async_trait::async_trait;

use crate::executor::ports::inbound::ExecutorRunPort;
use crate::shared::models::ExecutorInput;
use crate::shared::ports::ExecutorPort;

pub struct Executor {
    use_case: Box<dyn ExecutorRunPort>,
}

impl Executor {
    pub fn new(use_case: Box<dyn ExecutorRunPort>) -> Self {
        Self { use_case }
    }
}

#[async_trait]
impl ExecutorPort for Executor {
    async fn execute(&self, input: ExecutorInput) -> Result<()> {
        self.use_case.execute(input).await
    }
}
