use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::artifacts::AssembledPrompt;
use crate::interfaces::state::SessionConfig;
use crate::llm::ports::LlmGateway;

pub struct OpenAiAdapter {
    pub config: SessionConfig,
    pub client: reqwest::Client,
}

impl OpenAiAdapter {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmGateway for OpenAiAdapter {
    async fn call(&self, prompt: AssembledPrompt) -> Result<String> {
        todo!()
    }
}
