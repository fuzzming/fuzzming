use crate::shared::models::{AssembledPrompt, BugInfo, CoverageContext, GapType, Message, Role};
use super::fuzz_output_parser::format_for_llm;

const RULES: [&str; 5] = [
    "NO FOR-IN LOOPS: Solidity mappings are not iterable. Track actors with `address[] public actors` and push new callers into it.",
    "GHOST STATE: Ghost variables must track the raw INPUTS you send to the target (amounts deposited, shares withdrawn) — never the contract's own return values. Invariants then assert that the contract's reported state equals what the ghost says it should be. Example: ghost_deposits += amount; ghost_withdrawals += shares; invariant checks vault.totalAssets() == ghost_deposits - ghost_withdrawals. Tracking the contract's return value as ghost state is circular — if the contract reports a wrong value, the ghost mirrors the bug and the invariant is blind to it.",
    "NO HALLUCINATIONS: Only call functions and access public variables that are present in SOURCE_CODE. Verify each reference against the source before writing it.",
    "HANDLERS ARE WRAPPERS: Handler functions call the target contract externally — never reimplement its internal logic.",
    "NO TRY/CATCH: Never wrap target contract calls in try/catch. Pre-check conditions before calling (e.g. if (balance == 0) return; amount = bound(amount, 1, remaining);) so expected reverts are avoided up front. Unexpected reverts from the target must propagate — they signal bugs.",
];

pub struct Prompt {
    source_code: String,
    round: u32,
    fuzz_output: Option<String>,
    coverage_context: Option<CoverageContext>,
    confirmed_bugs: Vec<BugInfo>,
}

impl Prompt {
    pub fn new(
        round: u32,
        source_code: String,
        fuzz_output: Option<String>,
        coverage_context: Option<CoverageContext>,
        confirmed_bugs: Vec<BugInfo>,
    ) -> Self {
        Self { source_code, round, fuzz_output, coverage_context, confirmed_bugs }
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

        if !self.confirmed_bugs.is_empty() {
            let list = self
                .confirmed_bugs
                .iter()
                .map(|b| format!("  - {}", b.invariant_name))
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!(
                "CONFIRMED BUGS (these invariants already caught real vulnerabilities — keep them in the test and focus on finding additional distinct bugs):\n{}",
                list
            ));
        }

        if let Some(output) = &self.fuzz_output {
            sections.push(format!("FUZZ OUTPUT:\n{}", format_for_llm(output)));
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
        if !self.confirmed_bugs.is_empty() {
            context_sections.push("confirmed_bugs".to_string());
        }
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
            coverage.line_hit, coverage.line_found, line_pct,
            coverage.branch_hit, coverage.branch_found, branch_pct,
            coverage.function_hit, coverage.function_found, fn_pct,
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

#[cfg(test)]
mod tests {
    use super::Prompt;
    use crate::shared::models::{BugInfo, CoverageContext, CoverageGap, GapType, Role};

    #[test]
    fn assembled_prompt_includes_context_sections_and_messages() {
        let coverage = CoverageContext {
            gaps: vec![CoverageGap {
                file: "src/MyContract.sol".to_string(),
                line: 42,
                gap_type: GapType::Line,
                source_context: vec!["42: gap".to_string()],
            }],
            line_found: 10,
            line_hit: 2,
            branch_found: 0,
            branch_hit: 0,
            function_found: 0,
            function_hit: 0,
        };

        let assembled = Prompt::new(
            2,
            "contract C {}".to_string(),
            Some("revert".to_string()),
            Some(coverage),
            vec![],
        )
        .into_assembled();

        assert_eq!(assembled.messages.len(), 2);
        assert!(matches!(assembled.messages[0].role, Role::System));
        assert!(matches!(assembled.messages[1].role, Role::User));
        assert!(assembled.context_sections.contains(&"fuzz_output".to_string()));
        assert!(assembled.context_sections.contains(&"coverage".to_string()));
        assert!(assembled.messages[1].content.contains("Round: 2"));
        assert!(assembled.messages[1].content.contains("FUZZ OUTPUT"));
        assert!(assembled.messages[1].content.contains("COVERAGE SUMMARY"));
    }

    #[test]
    fn round_one_prompt_includes_full_generation_instruction() {
        let assembled =
            Prompt::new(1, "contract C {}".to_string(), None, None, vec![]).into_assembled();

        assert!(assembled.messages[1]
            .content
            .contains("Generate the full handler and invariant test suite"));
        assert!(assembled.context_sections.is_empty());
    }

    #[test]
    fn confirmed_bugs_appear_in_prompt_and_context_sections() {
        let bug = BugInfo {
            invariant_name: "invariant_solvency".to_string(),
            call_sequence: "handler_deposit()".to_string(),
        };
        let assembled =
            Prompt::new(3, "contract C {}".to_string(), None, None, vec![bug]).into_assembled();

        assert!(assembled.context_sections.contains(&"confirmed_bugs".to_string()));
        assert!(assembled.messages[1].content.contains("CONFIRMED BUGS"));
        assert!(assembled.messages[1].content.contains("invariant_solvency"));
    }
}
