use crate::shared::models::{CoverageContext, CoverageGap, GapType};
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::{HashMap, HashSet};

pub fn parse_lcov(lcov_data: &str) -> Result<CoverageContext> {
    let sf_re = Regex::new(r"^SF:(.+)").unwrap();
    let da_re = Regex::new(r"^DA:(\d+),(\d+)").unwrap();
    let br_re = Regex::new(r"^BRDA:(\d+),([0-9]+),([0-9-]+),([0-9-]+)").unwrap();
    let fn_re = Regex::new(r"^FN:(\d+),(.+)").unwrap();
    let fnda_re = Regex::new(r"^FNDA:([0-9-]+),(.+)").unwrap();
    let lf_re = Regex::new(r"^LF:(\d+)").unwrap();
    let lh_re = Regex::new(r"^LH:(\d+)").unwrap();
    let brf_re = Regex::new(r"^BRF:(\d+)").unwrap();
    let brh_re = Regex::new(r"^BRH:(\d+)").unwrap();
    let fnf_re = Regex::new(r"^FNF:(\d+)").unwrap();
    let fnh_re = Regex::new(r"^FNH:(\d+)").unwrap();

    let mut gaps: Vec<CoverageGap> = Vec::new();
    let mut current_sf: Option<String> = None;
    let mut fn_lines: HashMap<String, u32> = HashMap::new();
    let mut non_function_gap_lines: HashSet<(String, u32)> = HashSet::new();

    let mut line_found = 0;
    let mut line_hit = 0;
    let mut branch_found = 0;
    let mut branch_hit = 0;
    let mut function_found = 0;
    let mut function_hit = 0;

    let parse_hits = |raw: &str, line: &str| -> Result<u32> {
        if raw == "-" {
            Ok(0)
        } else {
            raw.parse::<u32>()
                .with_context(|| format!("Invalid hit count '{raw}' in LCOV line: {line}"))
        }
    };

    for line in lcov_data.lines() {
        if let Some(cap) = sf_re.captures(line) {
            current_sf = Some(cap[1].trim().to_string());
            fn_lines.clear();
            continue;
        }

        if let Some(cap) = lf_re.captures(line) {
            line_found += cap[1]
                .parse::<u32>()
                .with_context(|| format!("Invalid LF count in LCOV line: {line}"))?;
            continue;
        }

        if let Some(cap) = lh_re.captures(line) {
            line_hit += cap[1]
                .parse::<u32>()
                .with_context(|| format!("Invalid LH count in LCOV line: {line}"))?;
            continue;
        }

        if let Some(cap) = brf_re.captures(line) {
            branch_found += cap[1]
                .parse::<u32>()
                .with_context(|| format!("Invalid BRF count in LCOV line: {line}"))?;
            continue;
        }

        if let Some(cap) = brh_re.captures(line) {
            branch_hit += cap[1]
                .parse::<u32>()
                .with_context(|| format!("Invalid BRH count in LCOV line: {line}"))?;
            continue;
        }

        if let Some(cap) = fnf_re.captures(line) {
            function_found += cap[1]
                .parse::<u32>()
                .with_context(|| format!("Invalid FNF count in LCOV line: {line}"))?;
            continue;
        }

        if let Some(cap) = fnh_re.captures(line) {
            function_hit += cap[1]
                .parse::<u32>()
                .with_context(|| format!("Invalid FNH count in LCOV line: {line}"))?;
            continue;
        }

        if let Some(sf) = &current_sf {
            if let Some(cap) = fn_re.captures(line) {
                let line_num = cap[1]
                    .parse::<u32>()
                    .with_context(|| format!("Invalid FN line number in LCOV line: {line}"))?;
                let name = cap[2].trim().to_string();
                fn_lines.insert(name, line_num);
                continue;
            }

            if let Some(cap) = da_re.captures(line) {
                let hits: u32 = cap[2]
                    .parse()
                    .with_context(|| format!("Invalid DA hit count in LCOV line: {line}"))?;
                if hits == 0 {
                    let line_num = cap[1]
                        .parse()
                        .with_context(|| format!("Invalid DA line number in LCOV line: {line}"))?;
                    non_function_gap_lines.insert((sf.clone(), line_num));
                    gaps.push(CoverageGap {
                        file: sf.clone(),
                        line: line_num,
                        gap_type: GapType::Line,
                        source_context: Vec::new(),
                    });
                }
                continue;
            }
            if let Some(cap) = br_re.captures(line) {
                let hits: u32 = parse_hits(&cap[4], line)?;
                if hits == 0 {
                    let line_num = cap[1].parse().with_context(|| {
                        format!("Invalid BRDA line number in LCOV line: {line}")
                    })?;
                    non_function_gap_lines.insert((sf.clone(), line_num));
                    gaps.push(CoverageGap {
                        file: sf.clone(),
                        line: line_num,
                        gap_type: GapType::Branch,
                        source_context: Vec::new(),
                    });
                }
                continue;
            }
            if let Some(cap) = fnda_re.captures(line) {
                let hits: u32 = parse_hits(&cap[1], line)?;
                if hits == 0 {
                    let fn_name = cap[2].trim();
                    let line_num = fn_lines.get(fn_name).copied().unwrap_or(0);
                    if line_num == 0 || non_function_gap_lines.contains(&(sf.clone(), line_num)) {
                        continue;
                    }
                    gaps.push(CoverageGap {
                        file: sf.clone(),
                        line: line_num,
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
    let mut seen: HashSet<(String, u32, &'static str)> = HashSet::new();
    gaps.retain(|gap| {
        let kind = match gap.gap_type {
            GapType::Line => "line",
            GapType::Branch => "branch",
            GapType::Function => "function",
        };
        seen.insert((gap.file.clone(), gap.line, kind))
    });

    Ok(CoverageContext {
        gaps,
        line_found,
        line_hit,
        branch_found,
        branch_hit,
        function_found,
        function_hit,
    })
}
