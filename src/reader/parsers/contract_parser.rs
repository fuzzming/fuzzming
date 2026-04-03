use anyhow::Result;
use crate::interfaces::contexts::ContractContext;
use regex::Regex;

/// Simple, extendable contract parser using regex heuristics.
/// If `include_comments` is false, block and line comments are removed before parsing to reduce noise.
pub fn parse_contract(source: &str, include_comments: bool) -> Result<ContractContext> {
    let cleaned = if include_comments {
        source.to_string()
    } else {
        let re_block = Regex::new(r"/\*[\s\S]*?\*/").unwrap();
        let re_line = Regex::new(r"//.*").unwrap();
        let tmp = re_block.replace_all(source, "");
        re_line.replace_all(&tmp, "").to_string()
    };

    let contract_re = Regex::new(r"contract\s+([A-Za-z0-9_]+)").unwrap();
    let contract_name = contract_re
        .captures(&cleaned)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_else(|| "UnknownContract".to_string());

    let fn_re = Regex::new(r"(?:function|fn)\s+([A-Za-z0-9_]+)\s*\([^)]*\)").unwrap();
    let mut functions = Vec::new();
    for cap in fn_re.captures_iter(&cleaned) { if let Some(m) = cap.get(1) { functions.push(m.as_str().to_string()); } }

    let state_re = Regex::new(r"(?:uint256|uint|address|bool|mapping\s*\([^)]*\)|bytes[0-9]*|string)\s+([A-Za-z0-9_]+)\s*;").unwrap();
    let mut state_variables = Vec::new();
    for cap in state_re.captures_iter(&cleaned) { if let Some(m) = cap.get(1) { state_variables.push(m.as_str().to_string()); } }

    let mod_re = Regex::new(r"modifier\s+([A-Za-z0-9_]+)\b").unwrap();
    let mut modifiers = Vec::new();
    for cap in mod_re.captures_iter(&cleaned) { if let Some(m) = cap.get(1) { modifiers.push(m.as_str().to_string()); } }

    let const_re = Regex::new(r"([A-Za-z0-9_]+)\s+constant\s+[A-Za-z0-9_<>\[\]]+").unwrap();
    let mut constants = Vec::new();
    for cap in const_re.captures_iter(&cleaned) { if let Some(m) = cap.get(1) { constants.push(m.as_str().to_string()); } }

    Ok(ContractContext { 
        functions, 
        state_variables, 
        modifiers, 
        constants, 
        contract_name,
        source_code: cleaned,
    })
}
