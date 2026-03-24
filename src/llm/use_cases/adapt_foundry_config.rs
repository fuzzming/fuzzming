use anyhow::Result;
use crate::interfaces::artifacts::{AssembledPrompt, FoundryConfig};
use crate::llm::ports::LlmGateway;
use crate::llm::parsers::config_parser::parse_foundry_config;

pub async fn adapt_foundry_config(
    prompt: AssembledPrompt,
    gateway: &dyn LlmGateway,
) -> Result<FoundryConfig> {
    let raw = gateway.call(prompt).await?;
    parse_foundry_config(&raw)
}
