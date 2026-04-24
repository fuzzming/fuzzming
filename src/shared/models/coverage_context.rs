use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GapType {
    Line,
    Branch,
    Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageGap {
    pub file: String,
    pub line: u32,
    pub gap_type: GapType,
    pub source_context: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageContext {
    pub gaps: Vec<CoverageGap>,
    pub line_found: u32,
    pub line_hit: u32,
    pub branch_found: u32,
    pub branch_hit: u32,
    pub function_found: u32,
    pub function_hit: u32,
}
