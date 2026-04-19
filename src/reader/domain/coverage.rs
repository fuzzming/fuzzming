use crate::shared::models::{CoverageContext, CoverageGap, GapType};
use anyhow::Result;
use regex::Regex;
use std::collections::{HashMap, HashSet};

pub struct Coverage {
    gaps: Vec<CoverageGap>,
}

impl Coverage {
    pub fn from_lcov(lcov_data: &str) -> Result<Self> {
        let sf_re = Regex::new(r"^SF:(.+)").unwrap();
        let fn_re = Regex::new(r"^FN:(\d+),(.+)").unwrap();
        let da_re = Regex::new(r"^DA:(\d+),(\d+)").unwrap();
        let br_re = Regex::new(r"^BRDA:(\d+),\d+,[\d-]+,(-|\d+)").unwrap();
        let fnda_re = Regex::new(r"^FNDA:(\d+),(.+)").unwrap();

        let mut gaps: Vec<CoverageGap> = Vec::new();
        let mut current_sf: Option<String> = None;
        let mut fn_lines: HashMap<String, u32> = HashMap::new();
        let mut seen_branches: HashSet<(String, u32)> = HashSet::new();

        for line in lcov_data.lines() {
            if let Some(cap) = sf_re.captures(line) {
                current_sf = Some(cap[1].trim().to_string());
                fn_lines.clear();
                seen_branches.clear();
                continue;
            }

            if line.trim() == "end_of_record" {
                current_sf = None;
                continue;
            }

            let sf = match current_sf.as_deref() {
                Some(s) => s,
                None => continue,
            };

            // FN: line_no,fn_name — build name→line map for FNDA resolution
            if let Some(cap) = fn_re.captures(line) {
                let line_no: u32 = cap[1].parse().unwrap_or(0);
                fn_lines.insert(cap[2].trim().to_string(), line_no);
                continue;
            }

            // DA: line,hits — line coverage
            if let Some(cap) = da_re.captures(line) {
                let line_no: u32 = cap[1].parse().unwrap_or(0);
                let hits: u32 = cap[2].parse().unwrap_or(0);
                if hits == 0 {
                    gaps.push(CoverageGap {
                        file: sf.to_string(),
                        line: line_no,
                        gap_type: GapType::Line,
                        source_context: Vec::new(),
                    });
                }
                continue;
            }

            // BRDA: line,block,branch,taken — deduplicated per (file, line)
            if let Some(cap) = br_re.captures(line) {
                let line_no: u32 = cap[1].parse().unwrap_or(0);
                let hits: u32 = cap[2].parse().unwrap_or(0); // `-` → 0 → uncovered
                if hits == 0 {
                    let key = (sf.to_string(), line_no);
                    if seen_branches.insert(key) {
                        gaps.push(CoverageGap {
                            file: sf.to_string(),
                            line: line_no,
                            gap_type: GapType::Branch,
                            source_context: Vec::new(),
                        });
                    }
                }
                continue;
            }

            // FNDA: hits,fn_name — line resolved from FN records
            if let Some(cap) = fnda_re.captures(line) {
                let hits: u32 = cap[1].parse().unwrap_or(0);
                let fn_name = cap[2].trim();
                if hits == 0 {
                    let line_no = fn_lines.get(fn_name).copied().unwrap_or(0);
                    gaps.push(CoverageGap {
                        file: sf.to_string(),
                        line: line_no,
                        gap_type: GapType::Function,
                        source_context: Vec::new(),
                    });
                }
                continue;
            }
        }

        Ok(Self { gaps })
    }

    pub fn gaps_mut(&mut self) -> &mut Vec<CoverageGap> {
        &mut self.gaps
    }

    pub fn into_context(self) -> CoverageContext {
        CoverageContext { gaps: self.gaps }
    }
}
