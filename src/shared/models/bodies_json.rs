use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};

/// Accepts either a JSON string or a JSON array of strings.
/// LLMs sometimes send `"line1; line2;"` instead of `["line1", "line2"]`.
fn string_or_vec<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("string or array of strings")
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            Ok(vec![v.to_string()])
        }
        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Vec<String>, E> {
            Ok(vec![v])
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Vec<String>, A::Error> {
            let mut out = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                out.push(s);
            }
            Ok(out)
        }
    }
    d.deserialize_any(Visitor)
}

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
    /// Set automatically from the source file — LLM must not include this field.
    #[serde(default)]
    pub solidity: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandlerBodies {
    pub contract_name: String,
    pub imports: Vec<String>,
    /// Full Solidity contract definitions for mock/helper contracts placed before the Handler.
    #[serde(default)]
    pub helper_contracts: Vec<String>,
    pub state_vars: Vec<String>,
    pub ghost_vars: Vec<String>,
    pub constructor_signature: String,
    #[serde(deserialize_with = "string_or_vec")]
    pub constructor_body: Vec<String>,
    /// Ordered map — insertion order is preserved to keep targetSelectors weights stable.
    pub functions: IndexMap<String, String>,
    pub target_selectors: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvariantTestBodies {
    pub contract_name: String,
    pub imports: Vec<String>,
    pub state_vars: Vec<String>,
    #[serde(deserialize_with = "string_or_vec")]
    pub set_up_body: Vec<String>,
    /// Ordered map — insertion order preserved so generated Solidity is stable.
    pub invariants: IndexMap<String, String>,
}
