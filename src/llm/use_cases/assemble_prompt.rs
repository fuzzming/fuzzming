use anyhow::Result;
use crate::interfaces::artifacts::AssembledPrompt;
use crate::interfaces::contexts::{ContractContext, FuzzReportContext, CoverageContext};

pub fn assemble_prompt(
    round: u32,
    contract_context: ContractContext,
    fuzz_report_context: Option<FuzzReportContext>,
    coverage_context: Option<CoverageContext>,
) -> Result<AssembledPrompt> {
    todo!()
}
