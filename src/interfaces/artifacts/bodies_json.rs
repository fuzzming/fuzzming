use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Top-level artifact produced by the LLM each round.
/// Every value is already valid Solidity — the generator assembles .sol files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodiesJson {
    pub meta: BodiesMeta,
    pub handler: HandlerBodies,
    #[serde(rename = "invariantTest")]
    pub invariant_test: InvariantTestBodies,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodiesMeta {
    pub contract: String,
    pub contract_path: String,
    pub solidity: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandlerBodies {
    pub contract_name: String,
    /// Relative path from workspace root, e.g. "test/handlers/VaultHandler.sol"
    pub output_path: String,
    pub imports: Vec<String>,
    pub state_vars: Vec<String>,
    pub ghost_vars: Vec<String>,
    pub constructor_signature: String,
    pub constructor_body: Vec<String>,
    /// Ordered map — insertion order is preserved to keep targetSelectors weights stable.
    pub functions: IndexMap<String, String>,
    pub target_selectors: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvariantTestBodies {
    pub contract_name: String,
    /// Relative path from workspace root, e.g. "test/invariants/VaultInvariantTest.sol"
    pub output_path: String,
    pub imports: Vec<String>,
    pub state_vars: Vec<String>,
    pub set_up_body: Vec<String>,
    /// Ordered map — insertion order preserved so generated Solidity is stable.
    pub invariants: IndexMap<String, String>,
}
