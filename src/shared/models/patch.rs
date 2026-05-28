use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonBlockUpdate {
    #[serde(default)]
    pub op: JsonPatchOp,
    /// Dot-separated path into the JSON artifact.
    /// Examples: `"handler.functions.deposit"`, `"handler.ghostVars.0"`, `"handler.stateVars[1]"`
    pub path: String,
    /// New value (ignored for `Remove`). Also accepts `"body"` key that some LLMs use for function bodies.
    #[serde(alias = "body")]
    pub value: Value,
    /// LLM-provided justification for this change.
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum JsonPatchOp {
    Add,
    #[serde(alias = "modify")]
    #[default]
    Replace,
    Remove,
}
