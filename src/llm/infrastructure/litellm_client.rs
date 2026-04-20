use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use litellm_rs::{completion, system_message, user_message, CompletionOptions};
use serde_json::json;

use crate::llm::domain::llm_generation_response::LlmUsage;
use crate::llm::ports::LlmClientPort;

pub struct LiteLlmClient {
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

impl LiteLlmClient {
    pub fn new(
        model: impl Into<String>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Self {
        Self {
            model: model.into(),
            temperature,
            max_tokens,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    async fn call(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<(String, Option<LlmUsage>)> {
        let response = completion(
            &self.model,
            vec![
                system_message(system_prompt.to_string()),
                user_message(user_prompt.to_string()),
            ],
            Some(self.build_options()),
        )
        .await
        .map_err(|e| anyhow!("litellm completion failed: {e}"))?;

        let usage = response.usage.as_ref().map(|usage| LlmUsage {
            calls: 1,
            prompt_tokens: usage.prompt_tokens as u64,
            completion_tokens: usage.completion_tokens as u64,
            total_tokens: usage.total_tokens as u64,
            cached_prompt_tokens: usage
                .prompt_tokens_details
                .as_ref()
                .and_then(|d| d.cached_tokens)
                .unwrap_or(0) as u64,
            reasoning_tokens: usage
                .completion_tokens_details
                .as_ref()
                .and_then(|d| d.reasoning_tokens)
                .unwrap_or(0) as u64,
            thinking_tokens: usage
                .thinking_usage
                .as_ref()
                .and_then(|t| t.thinking_tokens)
                .unwrap_or(0) as u64,
        });

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .map(|content| content.to_string())
            .ok_or_else(|| anyhow!("LLM returned empty content"))?;

        Ok((content, usage))
    }
}

#[async_trait]
impl LlmClientPort for LiteLlmClient {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<(String, Option<LlmUsage>)> {
        self.call(system_prompt, user_prompt).await
    }
}

impl LiteLlmClient {
    fn build_options(&self) -> CompletionOptions {
        let mut options = CompletionOptions::default();
        options.temperature = self.temperature;
        options.max_tokens = self.max_tokens;
        options.extra_params = HashMap::from([(
            "response_format".to_string(),
            json!({ "type": "json_object" }),
        )]);
        options
    }
}
