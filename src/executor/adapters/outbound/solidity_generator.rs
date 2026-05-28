use crate::executor::adapters::outbound::FileSystemWriter;
use crate::executor::ports::outbound::CodeGeneratorPort;
use crate::shared::models::BodiesJson;
use anyhow::Result;
use async_trait::async_trait;

pub struct SolidityGenerator;

#[async_trait]
impl CodeGeneratorPort for SolidityGenerator {
    async fn generate(&self, bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
        // Read the pragma from the original source file so the LLM cannot accidentally
        // change it to a different Solidity version in a patch round.
        let pragma = read_source_pragma(bodies, writer).await;
        generate_handler(bodies, writer, &pragma).await?;
        generate_invariant_test(bodies, writer, &pragma).await?;
        Ok(())
    }
}

/// Extract `pragma solidity <version>` from the target contract source file.
/// Falls back to `bodies.meta.solidity` if the file cannot be read.
async fn read_source_pragma(bodies: &BodiesJson, writer: &FileSystemWriter) -> String {
    let path = writer.base_path().join(&bodies.meta.contract_path);
    if let Ok(source) = tokio::fs::read_to_string(&path).await {
        for line in source.lines() {
            let t = line.trim();
            if t.starts_with("pragma solidity") {
                return t
                    .trim_end_matches(';')
                    .trim_start_matches("pragma solidity")
                    .trim()
                    .to_string();
            }
        }
    }
    bodies.meta.solidity.clone()
}

fn needs_abi_encoder_v2(solidity: &str) -> bool {
    // forge-std's Test/StdInvariant uses string[] and struct arrays which require
    // ABIEncoderV2 in Solidity 0.7.x. Without this pragma the contract won't compile.
    solidity.contains("0.7.")
}

async fn generate_handler(bodies: &BodiesJson, writer: &FileSystemWriter, pragma: &str) -> Result<()> {
    let h = &bodies.handler;
    let mut out = Vec::<String>::new();

    out.push("// SPDX-License-Identifier: MIT".into());
    out.push(format!("pragma solidity {};", pragma));
    if needs_abi_encoder_v2(pragma) {
        out.push("pragma experimental ABIEncoderV2;".into());
    }
    out.push(String::new());

    for import in &h.imports {
        out.push(import.clone());
    }
    out.push(String::new());

    for helper in &h.helper_contracts {
        out.push(helper.clone());
        out.push(String::new());
    }

    out.push(format!("contract {} is Test {{", h.contract_name));
    out.push(String::new());

    for var in &h.state_vars {
        out.push(format!("    {var}"));
    }
    // ghost_vars is metadata only; declarations live in state_vars.
    out.push(String::new());

    let sig = h
        .constructor_signature
        .trim()
        .trim_end_matches('{')
        .trim_end();
    out.push(format!("    {sig} {{"));
    for stmt in &h.constructor_body {
        out.push(format!("        {stmt}"));
    }
    out.push("    }".into());
    out.push(String::new());

    // Skip hand-written functions that would conflict with public array getters.
    let auto_getters: std::collections::HashSet<String> = h
        .state_vars
        .iter()
        .filter(|v| v.contains("[]") && v.contains("public"))
        .filter_map(|v| {
            v.trim_end_matches(';')
                .split_whitespace()
                .last()
                .map(str::to_string)
        })
        .collect();

    for (name, fn_body) in &h.functions {
        if auto_getters.contains(name) {
            continue;
        }
        // Skip bare bodies without a function signature.
        if !fn_body.trim_start().starts_with("function ") {
            continue;
        }
        out.push(fn_body.clone());
        out.push(String::new());
    }

    out.push("}".into());

    let path = format!(
        "test/fuzzming/{}/{}.sol",
        bodies.meta.contract, h.contract_name
    );
    writer.write_file(&path, &out.join("\n")).await
}

async fn generate_invariant_test(bodies: &BodiesJson, writer: &FileSystemWriter, pragma: &str) -> Result<()> {
    let t = &bodies.invariant_test;
    let mut out = Vec::<String>::new();

    out.push("// SPDX-License-Identifier: MIT".into());
    out.push(format!("pragma solidity {};", pragma));
    if needs_abi_encoder_v2(pragma) {
        out.push("pragma experimental ABIEncoderV2;".into());
    }
    out.push(String::new());

    for import in &t.imports {
        out.push(import.clone());
    }
    out.push(String::new());

    out.push(format!("contract {} is Test {{", t.contract_name));
    out.push(String::new());

    for var in &t.state_vars {
        out.push(format!("    {var}"));
    }
    out.push(String::new());

    out.push("    function setUp() public {".into());
    for stmt in &t.set_up_body {
        out.push(format!("        {stmt}"));
    }
    out.push("    }".into());
    out.push(String::new());

    for inv_body in t.invariants.values() {
        if !inv_body.trim_start().starts_with("function ") {
            continue;
        }
        out.push(inv_body.clone());
        out.push(String::new());
    }

    out.push("}".into());

    let path = format!(
        "test/fuzzming/{}/{}.sol",
        bodies.meta.contract, t.contract_name
    );
    writer.write_file(&path, &out.join("\n")).await
}
