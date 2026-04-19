use anyhow::Result;
use async_trait::async_trait;

use crate::llm::adapters::litellm_generation_adapter::LiteLlmGenerationAdapter;
use crate::llm::ports::{LlmGenerationPort, LlmGenerationRequest, LlmGenerationResponse};

pub struct OpenRouterAdapter {
    inner: LiteLlmGenerationAdapter,
}

impl OpenRouterAdapter {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            inner: LiteLlmGenerationAdapter::new(
                "OPENROUTER_API_KEY",
                "openrouter",
                "openai/gpt-4o-mini",
                api_key,
                "openrouter adapter returned empty content",
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
impl LlmGenerationPort for OpenRouterAdapter {
    async fn generate(&self, request: LlmGenerationRequest) -> Result<LlmGenerationResponse> {
        self.inner.generate(request).await
    }
}
