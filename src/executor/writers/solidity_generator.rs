use crate::executor::infrastructure::FileSystemWriter;
use crate::interfaces::artifacts::BodiesJson;
use anyhow::Result;

pub async fn generate_handler(bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
    let h = &bodies.handler;
    let mut out = Vec::<String>::new();

    out.push("// SPDX-License-Identifier: MIT".into());
    out.push(format!("pragma solidity {};", bodies.meta.solidity));
    out.push(String::new());

    for import in &h.imports {
        out.push(import.clone());
    }
    out.push(String::new());

    out.push(format!("contract {} is BaseHandler {{", h.contract_name));
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

    writer.write_file(&h.output_path, &out.join("\n")).await
}

pub async fn generate_invariant_test(bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()> {
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

    writer.write_file(&t.output_path, &out.join("\n")).await
}
