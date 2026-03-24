use anyhow::Result;

/// Parses memory/history data from prior fuzzing sessions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    pub round: u32,
    pub notes: String,
}

pub fn parse_memory(data: &str) -> Result<Vec<MemoryEntry>> {
    todo!()
}
