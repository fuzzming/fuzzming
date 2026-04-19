use crate::shared::responses::{fuzz_report::FuzzPaths, session_outcome::TerminationReason};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationDecision {
    pub terminate: bool,
    pub reason: Option<TerminationReason>,
    pub paths: Option<FuzzPaths>,
}
