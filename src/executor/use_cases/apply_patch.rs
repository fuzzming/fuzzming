use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::shared::models::{JsonBlockUpdate, JsonPatchOp};

/// Apply a list of JSON patch operations to any serialisable value.
///
/// The value is round-tripped through [`serde_json::Value`], so dot-path keys
/// must match the **serialised** field names (camelCase where `#[serde(rename_all)]`
/// applies).
///
/// # Path syntax
/// - Dot-separated object keys: `"handler.functions.deposit"`
/// - Bare array index as last segment: `"handler.stateVars.0"`
/// - `key[N]` bracket syntax on a navigation segment: `"handler.ghostVars[0].name"`
pub fn apply_patches<T>(target: T, updates: &[JsonBlockUpdate]) -> Result<T>
where
    T: Serialize + DeserializeOwned,
{
    let mut root = serde_json::to_value(&target).context("serialise target")?;
    for update in updates {
        apply_update(&mut root, &update.path, &update.op, &update.value).with_context(|| {
            format!(
                "LLM patch failed: {} (op={:?}, path=\"{}\")",
                update.reason, update.op, update.path
            )
        })?;
    }
    serde_json::from_value(root).context("deserialise patched value")
}

fn apply_update(root: &mut Value, path: &str, op: &JsonPatchOp, new_val: &Value) -> Result<()> {
    if path.is_empty() {
        bail!("patch path is empty");
    }
    let segments: Vec<&str> = path.split('.').collect();
    let (parent_segs, tail) = segments.split_at(segments.len().saturating_sub(1));
    let last = tail.first().context("patch path is empty")?;

    let parent = navigate_mut(root, parent_segs)?;

    match op {
        JsonPatchOp::Add => match parent {
            Value::Object(map) => {
                // Treat Add as upsert: LLMs sometimes send Add for keys that already exist.
                map.insert(last.to_string(), new_val.clone());
            }
            Value::Array(arr) => {
                // last segment is ignored for arrays — value is appended
                let _ = last;
                arr.push(new_val.clone());
            }
            _ => bail!("expected object or array as parent"),
        },

        JsonPatchOp::Replace => match parent {
            Value::Object(map) => {
                map.insert(last.to_string(), new_val.clone());
            }
            Value::Array(arr) => {
                let idx = parse_index(last)?;
                let len = arr.len();
                *arr.get_mut(idx)
                    .with_context(|| format!("index {} out of bounds (len={})", idx, len))? =
                    new_val.clone();
            }
            _ => bail!("expected object or array as parent"),
        },

        JsonPatchOp::Remove => match parent {
            Value::Object(map) => {
                map.remove(*last)
                    .with_context(|| format!("key '{}' not found", last))?;
            }
            Value::Array(arr) => {
                let idx = parse_index(last)?;
                if idx >= arr.len() {
                    bail!("index {} out of bounds (len={})", idx, arr.len());
                }
                arr.remove(idx);
            }
            _ => bail!("expected object or array as parent"),
        },
    }

    Ok(())
}

fn navigate_mut<'a>(root: &'a mut Value, segments: &[&str]) -> Result<&'a mut Value> {
    let mut cur = root;
    for seg in segments {
        cur = step_into_mut(cur, seg)?;
    }
    Ok(cur)
}

/// Descend one segment. Supports `"key[N]"` for array elements inside objects.
fn step_into_mut<'a>(node: &'a mut Value, seg: &str) -> Result<&'a mut Value> {
    if let Some(bracket) = seg.find('[') {
        let key = &seg[..bracket];
        let idx_str = seg
            .get(bracket + 1..seg.len() - 1)
            .with_context(|| format!("malformed index in segment '{}'", seg))?;
        let idx = parse_index(idx_str)?;

        let child = match node {
            Value::Object(map) => map
                .get_mut(key)
                .with_context(|| format!("key '{}' not found", key))?,
            _ => bail!("expected object at segment '{}'", key),
        };
        match child {
            Value::Array(arr) => {
                let len = arr.len();
                arr.get_mut(idx).with_context(|| {
                    format!("'{}': index {} out of bounds (len={})", key, idx, len)
                })
            }
            _ => bail!("'{}' is not an array", key),
        }
    } else {
        match node {
            Value::Object(map) => map
                .get_mut(seg)
                .with_context(|| format!("key '{}' not found", seg)),
            Value::Array(arr) => {
                let idx = parse_index(seg)?;
                let len = arr.len();
                arr.get_mut(idx)
                    .with_context(|| format!("index {} out of bounds (len={})", idx, len))
            }
            _ => bail!("expected object or array at segment '{}'", seg),
        }
    }
}

fn parse_index(s: &str) -> Result<usize> {
    s.parse::<usize>()
        .with_context(|| format!("'{}' is not a valid array index", s))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use indexmap::IndexMap;
    use serde_json::json;

    use crate::shared::models::{
        BodiesJson, BodiesMeta, FoundryConfig, FuzzerConfigArtifact, HandlerBodies,
        InvariantTestBodies, JsonBlockUpdate, JsonPatchOp,
    };

    use super::apply_patches;

    fn upd(op: JsonPatchOp, path: &str, value: serde_json::Value) -> JsonBlockUpdate {
        JsonBlockUpdate {
            op,
            path: path.to_string(),
            value,
            reason: "test".to_string(),
        }
    }

    fn sample_bodies() -> BodiesJson {
        let mut functions = IndexMap::new();
        functions.insert("deposit".to_string(), "// deposit body".to_string());
        functions.insert("withdraw".to_string(), "// withdraw body".to_string());

        let mut invariants = IndexMap::new();
        invariants.insert("invariant_balance".to_string(), "assert(true);".to_string());

        BodiesJson {
            meta: BodiesMeta {
                contract: "Vault".to_string(),
                contract_path: "src/Vault.sol".to_string(),
                solidity: "^0.8.0".to_string(),
                generated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            handler: HandlerBodies {
                contract_name: "VaultHandler".to_string(),
                imports: vec!["import {Vault} from \"src/Vault.sol\";".to_string()],
                state_vars: vec!["Vault vault;".to_string()],
                ghost_vars: vec!["uint256 ghost_totalDeposited;".to_string()],
                constructor_signature: "constructor(address _vault)".to_string(),
                constructor_body: vec!["vault = Vault(_vault);".to_string()],
                functions,
                target_selectors: "targetSelector(...)".to_string(),
            },
            invariant_test: InvariantTestBodies {
                contract_name: "VaultInvariantTest".to_string(),
                imports: vec![],
                state_vars: vec![],
                set_up_body: vec![],
                invariants,
            },
        }
    }

    fn sample_config() -> FoundryConfig {
        let mut weights = HashMap::new();
        weights.insert("deposit".to_string(), 0.5_f64);
        weights.insert("withdraw".to_string(), 0.5_f64);
        FoundryConfig {
            depth: 100,
            runs: 256,
            seed: "0xdeadbeef".to_string(),
            max_test_rejects: 65536,
            dictionary_weight: 40,
            call_sequence_weights: weights,
            current_toml: None,
        }
    }

    #[test]
    fn replace_existing_function_body() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Replace,
                "handler.functions.deposit",
                json!("// new deposit body"),
            )],
        )
        .unwrap();
        assert_eq!(patched.handler.functions["deposit"], "// new deposit body");
        assert_eq!(patched.handler.functions["withdraw"], "// withdraw body");
    }

    #[test]
    fn add_new_function() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Add,
                "handler.functions.mint",
                json!("// mint body"),
            )],
        )
        .unwrap();
        assert_eq!(patched.handler.functions["mint"], "// mint body");
        assert_eq!(patched.handler.functions.len(), 3);
    }

    #[test]
    fn remove_function() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Remove,
                "handler.functions.withdraw",
                json!(null),
            )],
        )
        .unwrap();
        assert!(!patched.handler.functions.contains_key("withdraw"));
        assert_eq!(patched.handler.functions.len(), 1);
    }

    #[test]
    fn add_new_invariant() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Add,
                "invariantTest.invariants.invariant_solvency",
                json!("assert(solvency());"),
            )],
        )
        .unwrap();
        assert_eq!(
            patched.invariant_test.invariants["invariant_solvency"],
            "assert(solvency());"
        );
    }

    #[test]
    fn replace_meta_field() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(JsonPatchOp::Replace, "meta.solidity", json!("^0.8.20"))],
        )
        .unwrap();
        assert_eq!(patched.meta.solidity, "^0.8.20");
    }

    #[test]
    fn replace_array_element_by_index() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Replace,
                "handler.stateVars.0",
                json!("Vault newVault;"),
            )],
        )
        .unwrap();
        assert_eq!(patched.handler.state_vars[0], "Vault newVault;");
    }

    #[test]
    fn add_to_array_appends() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Add,
                "handler.ghostVars.end",
                json!("uint256 ghost_withdrawals;"),
            )],
        )
        .unwrap();
        assert_eq!(patched.handler.ghost_vars.len(), 2);
        assert_eq!(patched.handler.ghost_vars[1], "uint256 ghost_withdrawals;");
    }

    #[test]
    fn remove_array_element_by_index() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(JsonPatchOp::Remove, "handler.stateVars.0", json!(null))],
        )
        .unwrap();
        assert!(patched.handler.state_vars.is_empty());
    }

    #[test]
    fn multiple_patches_applied_in_order() {
        let patched = apply_patches(
            sample_bodies(),
            &[
                upd(JsonPatchOp::Add, "handler.functions.redeem", json!("// v1")),
                upd(
                    JsonPatchOp::Replace,
                    "handler.functions.redeem",
                    json!("// v2"),
                ),
            ],
        )
        .unwrap();
        assert_eq!(patched.handler.functions["redeem"], "// v2");
    }

    #[test]
    fn bracket_navigation_syntax() {
        // Exercise bracket syntax on a non-terminal segment.
        let mut bodies = sample_bodies();
        bodies
            .handler
            .ghost_vars
            .push("uint256 ghost_b;".to_string());
        let patched = apply_patches(
            bodies,
            &[upd(
                JsonPatchOp::Replace,
                "handler.ghostVars.1",
                json!("uint256 ghost_replaced;"),
            )],
        )
        .unwrap();
        assert_eq!(patched.handler.ghost_vars[1], "uint256 ghost_replaced;");
    }

    #[test]
    fn replace_depth() {
        let patched = apply_patches(
            sample_config(),
            &[upd(JsonPatchOp::Replace, "depth", json!(200))],
        )
        .unwrap();
        assert_eq!(patched.depth, 200);
    }

    #[test]
    fn replace_runs() {
        let patched = apply_patches(
            sample_config(),
            &[upd(JsonPatchOp::Replace, "runs", json!(512))],
        )
        .unwrap();
        assert_eq!(patched.runs, 512);
    }

    #[test]
    fn add_call_sequence_weight() {
        let patched = apply_patches(
            sample_config(),
            &[upd(
                JsonPatchOp::Add,
                "call_sequence_weights.redeem",
                json!(0.3),
            )],
        )
        .unwrap();
        assert!((patched.call_sequence_weights["redeem"] - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn replace_call_sequence_weight() {
        let patched = apply_patches(
            sample_config(),
            &[upd(
                JsonPatchOp::Replace,
                "call_sequence_weights.deposit",
                json!(0.7),
            )],
        )
        .unwrap();
        assert!((patched.call_sequence_weights["deposit"] - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn remove_call_sequence_weight() {
        let patched = apply_patches(
            sample_config(),
            &[upd(
                JsonPatchOp::Remove,
                "call_sequence_weights.withdraw",
                json!(null),
            )],
        )
        .unwrap();
        assert!(!patched.call_sequence_weights.contains_key("withdraw"));
    }

    #[test]
    fn patch_fuzzer_config_artifact() {
        let artifact = FuzzerConfigArtifact::Foundry(sample_config());
        let patched = apply_patches(
            artifact,
            &[upd(JsonPatchOp::Replace, "Foundry.depth", json!(500))],
        )
        .unwrap();
        let FuzzerConfigArtifact::Foundry(cfg) = patched;
        assert_eq!(cfg.depth, 500);
    }

    #[test]
    fn error_on_empty_path() {
        let result = apply_patches(
            sample_bodies(),
            &[upd(JsonPatchOp::Replace, "", json!("v"))],
        );
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("empty"), "unexpected error: {}", msg);
    }

    #[test]
    fn add_existing_key_upserts() {
        let patched = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Add,
                "handler.functions.deposit",
                json!("// updated deposit"),
            )],
        )
        .unwrap();
        assert_eq!(patched.handler.functions["deposit"], "// updated deposit");
    }

    #[test]
    fn error_on_remove_missing_key() {
        let result = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Remove,
                "handler.functions.nonexistent",
                json!(null),
            )],
        );
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("not found"), "unexpected error: {}", msg);
    }

    #[test]
    fn error_on_missing_intermediate_key() {
        let result = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Replace,
                "handler.nonexistentSection.key",
                json!("v"),
            )],
        );
        assert!(result.is_err());
    }

    #[test]
    fn error_on_array_index_out_of_bounds() {
        let result = apply_patches(
            sample_bodies(),
            &[upd(
                JsonPatchOp::Replace,
                "handler.stateVars.99",
                json!("x"),
            )],
        );
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("out of bounds"), "unexpected error: {}", msg);
    }
}
