use anyhow::Result;

use crate::generator::domain::Prompt;
use crate::shared::models::{AssembledPrompt, BugInfo, ContractContext, CoverageContext};

pub fn assemble_prompt(
    round: u32,
    contract_context: ContractContext,
    fuzz_output: Option<String>,
    coverage_context: Option<CoverageContext>,
    confirmed_bugs: Vec<BugInfo>,
    security_analysis: Option<String>,
) -> Result<AssembledPrompt> {
    Ok(Prompt::new(round, contract_context.source_code, fuzz_output, coverage_context, confirmed_bugs, security_analysis)
        .into_assembled())
}
