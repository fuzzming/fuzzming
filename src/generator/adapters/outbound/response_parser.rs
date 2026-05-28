use anyhow::{Context, Result};

use crate::generator::domain::generation_response::GenerationResponse;

pub fn extract_json_payload(raw: &str) -> Result<String> {
    let trimmed = raw.trim();

    // Find the first ```json or ``` fence anywhere in the response (handles prose preambles).
    let fence_start = trimmed.find("```json").or_else(|| trimmed.find("```"));

    if let Some(start) = fence_start {
        let after_fence = &trimmed[start..];
        let stripped = after_fence
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim();
        // Remove closing fence if present.
        let content = stripped
            .find("```")
            .map(|end| stripped[..end].trim())
            .unwrap_or(stripped);
        return Ok(content.to_string());
    }

    Ok(trimmed.to_string())
}

pub fn parse_generation_response(payload: &str) -> Result<GenerationResponse> {
    let mut value: serde_json::Value = serde_json::from_str(payload)
        .with_context(|| format!("failed to parse structured response: {payload}"))?;

    normalize_envelope(&mut value);

    serde_json::from_value(value)
        .with_context(|| format!("failed to parse structured response: {payload}"))
}

pub fn build_parse_repair_prompt(
    stage_name: &str,
    schema_hint: &str,
    previous_payload: &str,
    parse_error: &str,
) -> String {
    format!(
        "Your {stage_name} JSON is invalid. Repair it.\n\
         Return JSON only, no markdown.\n\
         Required shape: {schema_hint}\n\
         Parse error: {parse_error}\n\
         Invalid payload:\n{previous_payload}"
    )
}

fn normalize_envelope(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    let mode = obj
        .get("mode")
        .and_then(|m| m.as_str())
        .map(|m| m.to_string());

    match mode.as_deref() {
        Some("full") => {
            if let Some(full) = obj.remove("full") {
                if let Some(full_obj) = full.as_object() {
                    if !obj.contains_key("bodies") {
                        if let Some(bodies) = full_obj.get("bodies") {
                            obj.insert("bodies".to_string(), bodies.clone());
                        }
                    }
                    if !obj.contains_key("foundry_config") {
                        if let Some(foundry_config) = full_obj.get("foundry_config") {
                            obj.insert("foundry_config".to_string(), foundry_config.clone());
                        }
                    }
                }
            }
        }
        Some("patch") => {
            if let Some(patch) = obj.remove("patch") {
                if let Some(patch_obj) = patch.as_object() {
                    if !obj.contains_key("bodies_updates") {
                        if let Some(bodies_updates) = patch_obj.get("bodies_updates") {
                            obj.insert("bodies_updates".to_string(), bodies_updates.clone());
                        }
                    }
                    if !obj.contains_key("foundry_config_updates") {
                        if let Some(foundry_config_updates) =
                            patch_obj.get("foundry_config_updates")
                        {
                            obj.insert(
                                "foundry_config_updates".to_string(),
                                foundry_config_updates.clone(),
                            );
                        }
                    }
                }
            }
            normalize_array_patch_values(obj);
        }
        _ => {}
    }
}

/// Paths in bodies_updates whose values must be JSON arrays.
const ARRAY_PATHS: &[&str] = &[
    "handler.imports",
    "handler.stateVars",
    "handler.ghostVars",
    "handler.constructorBody",
    "invariantTest.imports",
    "invariantTest.stateVars",
    "invariantTest.setUpBody",
];

/// If the LLM sends a newline-joined string for a known array field, split it into an array.
fn normalize_array_patch_values(obj: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(updates) = obj.get_mut("bodies_updates").and_then(|v| v.as_array_mut()) else {
        return;
    };
    for update in updates.iter_mut() {
        let Some(update_obj) = update.as_object_mut() else {
            continue;
        };
        let path_is_array = update_obj
            .get("path")
            .and_then(|p| p.as_str())
            .map(|p| ARRAY_PATHS.contains(&p))
            .unwrap_or(false);

        if path_is_array {
            if let Some(serde_json::Value::String(s)) = update_obj.get("value") {
                let items: Vec<serde_json::Value> = s
                    .lines()
                    .map(|l| l.trim_end())
                    .filter(|l| !l.is_empty())
                    .map(|l| serde_json::Value::String(l.to_string()))
                    .collect();
                update_obj.insert("value".to_string(), serde_json::Value::Array(items));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markdown_code_fence() {
        let raw =
            "```json\n{\"mode\":\"patch\",\"bodies_updates\":[],\"foundry_config_updates\":[]}\n```";
        let out = extract_json_payload(raw).expect("must parse fence");
        assert_eq!(
            out,
            "{\"mode\":\"patch\",\"bodies_updates\":[],\"foundry_config_updates\":[]}"
        );
    }

    #[test]
    fn coerces_string_state_vars_to_array() {
        let payload = r#"{
            "mode": "patch",
            "bodies_updates": [
                {"op": "replace", "path": "handler.stateVars",
                 "value": "Vault public vault;\naddress public token;", "reason": "fix"}
            ],
            "foundry_config_updates": []
        }"#;
        let parsed = parse_generation_response(payload).expect("must parse");
        let GenerationResponse::Patch { bodies_updates, .. } = parsed else {
            panic!("expected patch");
        };
        assert!(
            bodies_updates[0].value.is_array(),
            "stateVars value should be coerced to array"
        );
        let arr = bodies_updates[0].value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], "Vault public vault;");
        assert_eq!(arr[1], "address public token;");
    }

    #[test]
    fn normalizes_nested_patch_envelope() {
        let payload = r#"{
            "mode": "patch",
            "patch": {
                "bodies_updates": [],
                "foundry_config_updates": []
            }
        }"#;

        let parsed = parse_generation_response(payload).expect("must parse normalized patch");
        assert!(matches!(parsed, GenerationResponse::Patch { .. }));
    }
}
