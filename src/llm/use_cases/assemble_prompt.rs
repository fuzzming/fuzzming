use crate::llm::domain::Prompt;
use crate::shared::models::{AssembledPrompt, ContractContext, CoverageContext};
use anyhow::Result;

pub fn assemble_prompt(
    round: u32,
    contract_context: ContractContext,
    fuzz_output: Option<String>,
    coverage_context: Option<CoverageContext>,
) -> Result<AssembledPrompt> {
    Ok(Prompt::new(round, contract_context.source_code, fuzz_output, coverage_context)
        .into_assembled())
}
