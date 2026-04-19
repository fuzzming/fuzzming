use crate::shared::models::InvariantFiles;
use crate::shared::models::RunnerResult;
use crate::shared::responses::fuzz_report::FuzzReport;
use anyhow::Result;

pub fn evaluate_outcome(
    fuzz_result: &RunnerResult,
    coverage_result: Option<&RunnerResult>,
    invariant_files: &InvariantFiles,
) -> Result<FuzzReport> {
    todo!()
}
