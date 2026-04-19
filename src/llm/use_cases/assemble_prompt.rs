use anyhow::Result;
use crate::interfaces::artifacts::AssembledPrompt;
use crate::interfaces::contexts::{ContractContext, CoverageContext};

pub fn assemble_prompt(
    round: u32,
    contract_context: ContractContext,
    fuzz_output: Option<String>,
    coverage_context: Option<CoverageContext>,
) -> Result<AssembledPrompt> {
    todo!()
}
