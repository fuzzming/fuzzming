use crate::shared::models::{CoverageContext, CoverageGap, GapType};
use anyhow::Result;
use regex::Regex;

/// Pure use case: parse lcov content and extract gaps.
pub fn parse_lcov(lcov_data: &str) -> Result<CoverageContext> {
    let sf_re = Regex::new(r"^SF:(.+)").unwrap();
    let da_re = Regex::new(r"^DA:(\d+),(\d+)").unwrap();
    let br_re = Regex::new(r"^BRDA:(\d+),([0-9]+),([0-9-]+),(\d+)").unwrap();
    let fnda_re = Regex::new(r"^FNDA:(\d+),(.+)").unwrap();

    let mut gaps: Vec<CoverageGap> = Vec::new();
    let mut current_sf: Option<String> = None;

    for line in lcov_data.lines() {
        if let Some(cap) = sf_re.captures(line) {
            current_sf = Some(cap[1].trim().to_string());
            continue;
        }

        if let Some(sf) = &current_sf {
            if let Some(cap) = da_re.captures(line) {
                let line_no: u32 = cap[1].parse().unwrap_or(0);
                let hits: u32 = cap[2].parse().unwrap_or(0);
                if hits == 0 {
                    gaps.push(CoverageGap {
                        file: sf.clone(),
                        line: line_no,
                        gap_type: GapType::Line,
                        source_context: Vec::new(),
                    });
                }
                continue;
            }
            if let Some(cap) = br_re.captures(line) {
                let line_no: u32 = cap[1].parse().unwrap_or(0);
                let hits: u32 = cap[4].parse().unwrap_or(0);
                if hits == 0 {
                    gaps.push(CoverageGap {
                        file: sf.clone(),
                        line: line_no,
                        gap_type: GapType::Branch,
                        source_context: Vec::new(),
                    });
                }
                continue;
            }
            if let Some(cap) = fnda_re.captures(line) {
                let hits: u32 = cap[1].parse().unwrap_or(0);
                if hits == 0 {
                    gaps.push(CoverageGap {
                        file: sf.clone(),
                        line: 0, // FNDA doesn't have a line number, might need adjustment
                        gap_type: GapType::Function,
                        source_context: Vec::new(),
                    });
                }
                continue;
            }
        }

        if line.trim() == "end_of_record" {
            current_sf = None;
        }
    }

    Ok(CoverageContext { gaps })
}
