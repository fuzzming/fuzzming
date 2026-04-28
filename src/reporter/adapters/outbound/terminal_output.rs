use anyhow::Result;
use async_trait::async_trait;

use crate::reporter::ports::outbound::OutputPort;

pub struct TerminalOutput;

impl TerminalOutput {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OutputPort for TerminalOutput {
    async fn write(&self, output: &str) -> Result<()> {
        println!("{}", output);
        Ok(())
    }
}
