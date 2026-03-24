use serde::{Deserialize, Serialize};
use crate::interfaces::signals::fuzz_report::FuzzPaths;
use crate::interfaces::signals::session_outcome::TerminationReason;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationDecision {
    pub terminate: bool,
    pub reason: Option<TerminationReason>,
    pub paths: Option<FuzzPaths>,
}
