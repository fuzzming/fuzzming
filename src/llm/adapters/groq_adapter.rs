use anyhow::Result;
use async_trait::async_trait;

use crate::llm::adapters::litellm_generation_adapter::LiteLlmGenerationAdapter;
use crate::llm::ports::{LlmGenerationPort, LlmGenerationRequest, LlmGenerationResponse};

pub struct GroqAdapter {
    inner: LiteLlmGenerationAdapter,
}

impl GroqAdapter {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            inner: LiteLlmGenerationAdapter::new(
                "GROQ_API_KEY",
                "groq",
                "openai/gpt-oss-120b",
                api_key,
                "groq adapter returned empty content",
            ),
        }
    }

    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.inner = self.inner.with_default_model(model);
        self
    }

    pub fn with_temperature(mut self, temperature: Option<f32>) -> Self {
        self.inner = self.inner.with_temperature(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: Option<u32>) -> Self {
        self.inner = self.inner.with_max_tokens(max_tokens);
        self
    }
}

#[async_trait]
impl LlmGenerationPort for GroqAdapter {
    async fn generate(&self, request: LlmGenerationRequest) -> Result<LlmGenerationResponse> {
        self.inner.generate(request).await
    }
}
