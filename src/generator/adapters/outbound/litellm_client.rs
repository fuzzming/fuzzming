use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use litellm_rs::{completion, system_message, user_message, CompletionOptions};
use serde_json::json;
use tracing::warn;

use crate::shared::models::GenerationUsage;
use crate::generator::ports::outbound::LlmClientPort;

const MAX_HTTP_RETRIES: u32 = 3;

pub struct LiteLlmClient {
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    timeout_secs: u64,
}

impl LiteLlmClient {
    pub fn new(
        model: impl Into<String>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        timeout_secs: u64,
    ) -> Self {
        Self {
            model: model.into(),
            temperature,
            max_tokens,
            timeout_secs,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    async fn call(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<(String, Option<GenerationUsage>)> {
        let response = tokio::time::timeout(
            Duration::from_secs(self.timeout_secs),
            completion(
                &self.model,
                vec![
                    system_message(system_prompt.to_string()),
                    user_message(user_prompt.to_string()),
                ],
                Some(self.build_options()),
            ),
        )
        .await
        .map_err(|_| anyhow!("LLM call timed out after {}s", self.timeout_secs))?
        .map_err(|e| anyhow!("litellm completion failed: {e}"))?;

        let usage = response.usage.as_ref().map(|usage| GenerationUsage {
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
    ) -> Result<(String, Option<GenerationUsage>)> {
        let mut last_err = anyhow!("no attempts made");
        for attempt in 1..=MAX_HTTP_RETRIES {
            match self.call(system_prompt, user_prompt).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let msg = err.to_string();
                    if !is_transient_error(&msg) || attempt == MAX_HTTP_RETRIES {
                        return Err(err);
                    }
                    let wait = Duration::from_secs(2u64.pow(attempt));
                    warn!(attempt, "transient LLM error, retrying in {}s: {}", wait.as_secs(), msg);
                    tokio::time::sleep(wait).await;
                    last_err = err;
                }
            }
        }
        Err(last_err)
    }
}

fn is_transient_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("429")
        || lower.contains("503")
        || lower.contains("502")
        || lower.contains("rate limit")
        || lower.contains("overloaded")
        || lower.contains("connection")
        || lower.contains("timed out")
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
