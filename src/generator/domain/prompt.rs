use crate::shared::models::{AssembledPrompt, CoverageContext, GapType, Message, Role};

const RULES: [&str; 4] = [
    "JSON ONLY: return valid JSON, no markdown.",
    "NO FOR-IN LOOPS: Solidity has no for-in; use actors[] for iteration.",
    "HANDLER NAME: must be Handler or [Target]Handler.",
    "CONFIG KEYS: call_sequence_weights keys must match handler.functions.",
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
        Self {
            source_code,
            round,
            fuzz_output,
            coverage_context,
        }
    }

    pub fn system_message(&self) -> String {
        let rules = RULES
            .iter()
            .enumerate()
            .map(|(i, r)| format!("{}. {}", i + 1, r))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Senior smart-contract security researcher and Foundry fuzzing expert.\n\
             Source code:\n{}\n\n\
             Rules:\n{}",
            self.source_code, rules
        )
    }

    pub fn user_message(&self) -> String {
        let mut sections: Vec<String> = vec![format!("Round: {}", self.round)];

        if let Some(output) = &self.fuzz_output {
            sections.push(format!("Fuzz output:\n{}", output));
        }

        if let Some(coverage) = &self.coverage_context {
            sections.push(Self::format_coverage(coverage));
        }

        if self.round == 1 {
            sections.push("Generate full handler + invariants.".to_string());
        } else {
            sections.push("Patch handler/invariants using fuzz + coverage.".to_string());
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
                Message {
                    role: Role::System,
                    content: system,
                },
                Message {
                    role: Role::User,
                    content: user,
                },
            ],
            round: self.round,
            context_sections,
        }
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
}
