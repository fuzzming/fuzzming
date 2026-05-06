use crate::executor::adapters::outbound::FileSystemWriter;
use crate::executor::ports::outbound::CodeGeneratorPort;
use crate::shared::models::BodiesJson;
use anyhow::Result;
use async_trait::async_trait;

pub struct SolidityGenerator;

#[async_trait]
impl CodeGeneratorPort for SolidityGenerator {
    async fn generate(&self, bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
        generate_handler(bodies, writer).await?;
        generate_invariant_test(bodies, writer).await?;
        Ok(())
    }
}

async fn generate_handler(bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
    let h = &bodies.handler;
    let mut out = Vec::<String>::new();

    out.push("// SPDX-License-Identifier: MIT".into());
    out.push(format!("pragma solidity {};", bodies.meta.solidity));
    out.push(String::new());

    for import in &h.imports {
        out.push(import.clone());
    }
    out.push(String::new());

    out.push(format!("contract {} {{", h.contract_name));
    out.push(String::new());

    for var in &h.state_vars {
        out.push(format!("    {}", var));
    }
    out.push(String::new());

    for ghost in &h.ghost_vars {
        out.push(format!("    {}", ghost));
    }
    out.push(String::new());

    out.push(format!("    {} {{", h.constructor_signature));
    for stmt in &h.constructor_body {
        out.push(format!("        {}", stmt));
    }
    out.push("    }".into());
    out.push(String::new());

    for fn_body in h.functions.values() {
        out.push(fn_body.clone());
        out.push(String::new());
    }

    out.push(h.target_selectors.clone());
    out.push(String::new());
    out.push("}".into());

    let path = format!("test/fuzzming/{}/{}.sol", bodies.meta.contract, h.contract_name);
    writer.write_file(&path, &out.join("\n")).await
}

async fn generate_invariant_test(bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
    let t = &bodies.invariant_test;
    let mut out = Vec::<String>::new();

    out.push("// SPDX-License-Identifier: MIT".into());
    out.push(format!("pragma solidity {};", bodies.meta.solidity));
    out.push(String::new());

    for import in &t.imports {
        out.push(import.clone());
    }
    out.push(String::new());

    out.push(format!("contract {} is Test {{", t.contract_name));
    out.push(String::new());

    for var in &t.state_vars {
        out.push(format!("    {}", var));
    }
    out.push(String::new());

    out.push("    function setUp() public {".into());
    for stmt in &t.set_up_body {
        out.push(format!("        {}", stmt));
    }
    out.push("    }".into());
    out.push(String::new());

    for inv_body in t.invariants.values() {
        out.push(inv_body.clone());
        out.push(String::new());
    }

    out.push("}".into());

    let path = format!("test/fuzzming/{}/{}.sol", bodies.meta.contract, t.contract_name);
    writer.write_file(&path, &out.join("\n")).await
}
