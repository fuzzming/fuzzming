use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;

use crate::shared::models::ReportArtifacts;
use crate::shared::ports::ReporterReaderPort;

pub struct FileArtifactReader {
    workspace_root: PathBuf,
}

impl FileArtifactReader {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[async_trait]
impl ReporterReaderPort for FileArtifactReader {
    async fn get_report_artifacts(&self, contract_name: &str) -> Result<ReportArtifacts> {
        let contract_dir = self.workspace_root.join(".fuzzming").join(contract_name);

        let fuzz_output = match fs::read_to_string(contract_dir.join("fuzz_output.txt")).await {
            Ok(s) => s,
            Err(_) => String::new(),
        };

        let call_sequences = extract_call_sequences(&fuzz_output);

        let coverage_summary = match fs::read_to_string(contract_dir.join("lcov.info")).await {
            Ok(lcov) => Some(summarise_lcov(&lcov)),
            Err(_) => None,
        };

        Ok(ReportArtifacts {
            contract_name: contract_name.to_string(),
            fuzz_output,
            coverage_summary,
            call_sequences,
            round_history: 0, // overwritten by Reporter::emit from SessionOutcome
        })
    }
}

fn extract_call_sequences(output: &str) -> Vec<String> {
    let mut sequences = Vec::new();
    let mut in_sequence = false;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("call sequence:") || trimmed.starts_with("Call sequence:") {
            in_sequence = true;
            continue;
        }
        if in_sequence {
            if trimmed.is_empty() {
                in_sequence = false;
            } else {
                sequences.push(trimmed.to_string());
            }
        }
    }

    sequences
}

fn summarise_lcov(lcov: &str) -> String {
    let (mut lf, mut lh, mut brf, mut brh, mut fnf, mut fnh) = (0u32, 0u32, 0u32, 0u32, 0u32, 0u32);

    for line in lcov.lines() {
        if let Some(v) = parse_lcov_field(line, "LF:") { lf += v; }
        else if let Some(v) = parse_lcov_field(line, "LH:") { lh += v; }
        else if let Some(v) = parse_lcov_field(line, "BRF:") { brf += v; }
        else if let Some(v) = parse_lcov_field(line, "BRH:") { brh += v; }
        else if let Some(v) = parse_lcov_field(line, "FNF:") { fnf += v; }
        else if let Some(v) = parse_lcov_field(line, "FNH:") { fnh += v; }
    }

    format!(
        "Lines:     {}/{}\nBranches:  {}/{}\nFunctions: {}/{}",
        lh, lf, brh, brf, fnh, fnf
    )
}

fn parse_lcov_field(line: &str, prefix: &str) -> Option<u32> {
    line.strip_prefix(prefix)?.trim().parse().ok()
}
