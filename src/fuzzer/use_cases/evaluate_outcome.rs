use anyhow::Result;
use crate::interfaces::artifacts::RunnerResult;
use crate::interfaces::signals::FuzzReport;
use crate::interfaces::contexts::InvariantFiles;

pub fn evaluate_outcome(
    fuzz_result: &RunnerResult,
    coverage_result: Option<&RunnerResult>,
    invariant_files: &InvariantFiles,
) -> Result<FuzzReport> {
    todo!()
}
