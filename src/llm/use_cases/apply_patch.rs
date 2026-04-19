use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::llm::ports::JsonBlockUpdate;
use crate::shared::models::{BodiesJson, FoundryConfig};

pub fn apply_bodies_patch(bodies: BodiesJson, updates: &[JsonBlockUpdate]) -> Result<BodiesJson> {
    let mut value = serde_json::to_value(bodies)?;
    for update in updates {
        set_dot_path(&mut value, &update.path, update.value.clone())?;
    }
    serde_json::from_value(value).context("bodies patch produced invalid BodiesJson")
}

pub fn apply_config_patch(
    config: FoundryConfig,
    updates: &[JsonBlockUpdate],
) -> Result<FoundryConfig> {
    let mut value = serde_json::to_value(config)?;
    for update in updates {
        set_dot_path(&mut value, &update.path, update.value.clone())?;
    }
    serde_json::from_value(value).context("config patch produced invalid FoundryConfig")
}

fn set_dot_path(root: &mut Value, path: &str, new_value: Value) -> Result<()> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            match current {
                Value::Object(map) => {
                    map.insert(part.to_string(), new_value);
                    return Ok(());
                }
                _ => bail!("path `{}`: expected object at `{}`", path, part),
            }
        }
        current = match current {
            Value::Object(map) => map
                .get_mut(*part)
                .with_context(|| format!("path `{}`: key `{}` not found", path, part))?,
            _ => bail!("path `{}`: expected object at `{}`", path, part),
        };
    }

    Ok(())
}
