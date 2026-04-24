use crate::shared::models::{AssembledPrompt, CoverageContext, GapType, Message, Role};

const RULES: [&str; 5] = [
    "NO FOR-IN LOOPS: Solidity mappings are not iterable. Use a ghost array `address[] public actors` and push msg.sender to it.",
    "PHYSICAL VS LOGICAL: Always compare internal state (totalAssets) against physical balances (asset.balanceOf(address(this))).",
    "NAMESPACING: Handler contract must be named `Handler` or `[Target]Handler`.",
    "USE INDEXMAP ORDER: Generate JSON keys in the order they should appear in Solidity.",
    "OUTPUT: Return valid JSON only.",
];

pub struct Prompt {
    source_code: String,
    round: u32,
    fuzz_output: Option<String>,
    coverage_context: Option<CoverageContext>,
}

impl Prompt {
    pub fn new(
        round: u32,
        source_code: String,
        fuzz_output: Option<String>,
        coverage_context: Option<CoverageContext>,
    ) -> Self {
        Self { source_code, round, fuzz_output, coverage_context }
    }

    pub fn system_message(&self) -> String {
        let rules = RULES
            .iter()
            .enumerate()
            .map(|(i, r)| format!("{}. {}", i + 1, r))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "You are a Senior Smart-Contract Security Researcher and Foundry Fuzzing Expert.\n\
             SOURCE_CODE:\n{}\n\n\
             STRICT OPERATIONAL RULES:\n{}",
            self.source_code, rules
        )
    }

    pub fn user_message(&self) -> String {
        let mut sections: Vec<String> = vec![format!("Round: {}", self.round)];

        if let Some(output) = &self.fuzz_output {
            sections.push(format!("FUZZ OUTPUT:\n{}", output));
        }

        if let Some(coverage) = &self.coverage_context {
            sections.push(Self::format_coverage(coverage));
        }

        if self.round == 1 {
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

    pub fn into_assembled(self) -> AssembledPrompt {
        let mut context_sections = Vec::new();
        if self.fuzz_output.is_some() {
            context_sections.push("fuzz_output".to_string());
        }
        if self.coverage_context.is_some() {
            context_sections.push("coverage".to_string());
        }

        let system = self.system_message();
        let user = self.user_message();

        AssembledPrompt {
            messages: vec![
                Message { role: Role::System, content: system },
                Message { role: Role::User, content: user },
            ],
            round: self.round,
            context_sections,
        }
    }

    fn format_coverage(coverage: &CoverageContext) -> String {
        let line_pct = if coverage.line_found == 0 {
            100.0
        } else {
            (coverage.line_hit as f64 / coverage.line_found as f64) * 100.0
        };
        let branch_pct = if coverage.branch_found == 0 {
            100.0
        } else {
            (coverage.branch_hit as f64 / coverage.branch_found as f64) * 100.0
        };
        let fn_pct = if coverage.function_found == 0 {
            100.0
        } else {
            (coverage.function_hit as f64 / coverage.function_found as f64) * 100.0
        };

        let mut lines = vec![format!(
            "COVERAGE SUMMARY: lines {}/{} ({:.1}%), branches {}/{} ({:.1}%), functions {}/{} ({:.1}%)",
            coverage.line_hit,
            coverage.line_found,
            line_pct,
            coverage.branch_hit,
            coverage.branch_found,
            branch_pct,
            coverage.function_hit,
            coverage.function_found,
            fn_pct
        )];

        if coverage.gaps.is_empty() {
            lines.push("COVERAGE: Full coverage achieved.".to_string());
            return lines.join("\n");
        }

        lines.push("COVERAGE GAPS (never executed):".to_string());
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
}
