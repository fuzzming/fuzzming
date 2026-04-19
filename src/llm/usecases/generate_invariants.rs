use anyhow::Result;

use crate::interfaces::artifacts::{AssembledPrompt, InvariantSet};
use crate::llm::ports::LlmGateway;
use crate::llm::usecases::parsers::invariant_parser::parse_invariants;

pub async fn generate_invariants(
    prompt: AssembledPrompt,
    gateway: &dyn LlmGateway,
    target_file_path: String,
) -> Result<InvariantSet> {
    let raw = gateway.call(prompt).await?;
    parse_invariants(&raw, target_file_path)
}
