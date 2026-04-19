use crate::shared::models::{AssembledPrompt, Message, Role};
use crate::shared::models::{ContractContext, CoverageContext, GapType};
use anyhow::Result;

pub fn assemble_prompt(
    round: u32,
    contract_context: ContractContext,
    fuzz_output: Option<String>,
    coverage_context: Option<CoverageContext>,
) -> Result<AssembledPrompt> {
    let system = build_system_message(&contract_context.source_code);
    let user = build_user_message(round, &fuzz_output, &coverage_context);

    Ok(AssembledPrompt {
        messages: vec![
            Message {
                role: Role::System,
                content: system,
            },
            Message {
                role: Role::User,
                content: user,
            },
        ],
        round,
        context_sections: build_context_sections(&fuzz_output, &coverage_context),
    })
}

fn build_system_message(source_code: &str) -> String {
    format!(
        "You are a Senior Smart-Contract Security Researcher and Foundry Fuzzing Expert.\n\
         SOURCE_CODE:\n{}\n\n\
         STRICT OPERATIONAL RULES:\n\
         1. NO FOR-IN LOOPS: Solidity mappings are not iterable. Use a ghost array `address[] public actors` and push msg.sender to it.\n\
         2. PHYSICAL VS LOGICAL: Always compare internal state (totalAssets) against physical balances (asset.balanceOf(address(this))).\n\
         3. NAMESPACING: Handler contract must be named `Handler` or `[Target]Handler`.\n\
         4. USE INDEXMAP ORDER: Generate JSON keys in the order they should appear in Solidity.\n\
         5. OUTPUT: Return valid JSON only.",
        source_code
    )
}

fn build_user_message(
    round: u32,
    fuzz_output: &Option<String>,
    coverage_context: &Option<CoverageContext>,
) -> String {
    let mut sections: Vec<String> = Vec::new();

    sections.push(format!("Round: {}", round));

    if let Some(output) = fuzz_output {
        sections.push(format!("FUZZ OUTPUT:\n{}", output));
    }

    if let Some(coverage) = coverage_context {
        sections.push(format_coverage(coverage));
    }

    if round == 1 {
        sections.push(
            "Generate the full handler and invariant test suite for this contract.".to_string(),
        );
    } else {
        sections.push(
            "Based on the fuzz output and coverage gaps above, patch or rewrite the invariants and handler as needed.".to_string(),
        );
    }

    sections.join("\n\n")
}

fn format_coverage(coverage: &CoverageContext) -> String {
    if coverage.gaps.is_empty() {
        return "COVERAGE: Full coverage achieved.".to_string();
    }

    let mut lines = vec!["COVERAGE GAPS (never executed):".to_string()];
    for gap in &coverage.gaps {
        let kind = match gap.gap_type {
            GapType::Line => "line",
            GapType::Branch => "branch",
            GapType::Function => "function",
        };
        lines.push(format!("  [{kind}] {}:{}", gap.file, gap.line));
        for ctx in &gap.source_context {
            lines.push(format!("    {}", ctx));
        }
    }
    lines.join("\n")
}

fn build_context_sections(
    fuzz_output: &Option<String>,
    coverage_context: &Option<CoverageContext>,
) -> Vec<String> {
    let mut sections = Vec::new();
    if fuzz_output.is_some() {
        sections.push("fuzz_output".to_string());
    }
    if coverage_context.is_some() {
        sections.push("coverage".to_string());
    }
    sections
}
