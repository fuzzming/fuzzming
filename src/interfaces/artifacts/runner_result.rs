use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}
