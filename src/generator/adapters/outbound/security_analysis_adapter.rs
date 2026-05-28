use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

use super::response_parser::extract_json_payload;
use crate::generator::ports::outbound::LlmClientPort;
use crate::shared::models::BugInfo;
use crate::shared::ports::{SecurityAnalysisPort, SecurityAnalysisRequest};

pub struct LiteLlmSecurityAnalysisAdapter {
    client: Arc<dyn LlmClientPort>,
}

impl LiteLlmSecurityAnalysisAdapter {
    pub fn new(client: Arc<dyn LlmClientPort>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SecurityAnalysisPort for LiteLlmSecurityAnalysisAdapter {
    async fn analyze(&self, request: SecurityAnalysisRequest) -> Result<String> {
        let system = build_system_prompt();
        let user = build_user_prompt(&request);
        let (raw, _) = self.client.complete(&system, &user).await?;

        // Extract the "analysis" field if the model wrapped output in JSON.
        let payload = extract_json_payload(&raw).unwrap_or_else(|_| raw.clone());
        let llm_text = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&payload) {
            v.get("analysis")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
                .unwrap_or(raw)
        } else {
            raw
        };

        // Prepend confirmed bugs so the stored analysis is self-contained.
        // When this becomes `previous_analysis` next round, the LLM knows exactly
        // which bugs were confirmed at the time this analysis was written.
        let bugs_header = if request.confirmed_bugs.is_empty() {
            format!(
                "**Confirmed bugs at round {}:** None\n\n",
                request.rounds_completed
            )
        } else {
            let list = format_bugs(&request.confirmed_bugs);
            format!(
                "**Confirmed bugs at round {}:**\n{}\n\n",
                request.rounds_completed, list
            )
        };

        Ok(format!("{bugs_header}{llm_text}"))
    }
}

fn build_system_prompt() -> String {
    "You are an expert smart contract security auditor assisting a test generator in an \
     iterative fuzzing loop. Your output is guidance for the next generator call, not a \
     standalone audit report. Each round you receive the latest fuzzer output and an \
     optional previous analysis, and you produce a refined security analysis. Your job is to:\n\
     1. Explain confirmed fuzzing failures to the generator: root cause, affected code, \
        why the invariant failed, and what class of bug it proves.\n\
     2. Update confirmed bug findings — adjust severity or exploitability if new evidence warrants it.\n\
     3. Identify vulnerabilities the fuzzer is STILL missing — \
        focus on: access control, reentrancy, arithmetic overflow, oracle manipulation, \
        economic attacks (MEV/sandwich/frontrunning), state machine violations, \
        admin key risks, and missing validation.\n\
     Group findings by root cause, not by exploit path. If two call sequences, invariants, \
     or symptoms come from the same code defect, treat them as one finding and explicitly \
     say they share the same root cause.\n\
     Do not ask the generator to reproduce the same bug through another invariant, call \
     sequence, or symptom. Once a root cause is confirmed, direct the generator toward \
     other independent problems in the code.\n\
     Be precise: reference specific function names and the exact conditions that create risk. \
     Do NOT re-describe confirmed bugs in section 3 — list only distinct issues not yet caught, \
     and exclude near-duplicates that come from the same underlying problem.\n\
     Return JSON: {\"analysis\": \"<full markdown text>\"}".to_string()
}

fn build_user_prompt(request: &SecurityAnalysisRequest) -> String {
    let bugs_section = if request.confirmed_bugs.is_empty() {
        "None — the fuzzer ran but found no invariant violations.".to_string()
    } else {
        format_bugs(&request.confirmed_bugs)
    };

    let fuzz_section = match &request.fuzz_output {
        Some(output) => format!("\n\nLatest fuzz output:\n```\n{output}\n```"),
        None => String::new(),
    };

    let previous_section = match &request.previous_analysis {
        Some(prev) => format!("\n\nPrevious analysis (refine and extend this — do not discard valid findings):\n{prev}"),
        None => String::new(),
    };

    let instruction = if request.previous_analysis.is_some() {
        "Update the analysis based on the latest fuzz output:\n\
         ## Confirmed Failure Explanation for Generator\n\
         Explain each confirmed failure by root cause, affected function/state, why the \
         invariant failed, and what the generator should avoid repeating. Merge findings \
         that share the same root cause, even if they have different call sequences or \
         invariant names. Update severity if new evidence warrants it.\n\n\
         ## Independent Vulnerabilities to Try Next\n\
         Suggest only distinct issues the fuzzer is still missing. Do not list another \
         way to trigger an already-confirmed root cause."
    } else {
        "Provide an initial analysis:\n\
         ## Confirmed Failure Explanation for Generator\n\
         For each confirmed bug: root cause, affected function/state, why the invariant \
         failed, severity (Critical/High/Medium/Low), and exploitability without privileged \
         access. Merge duplicates by root cause.\n\n\
         ## Independent Vulnerabilities to Try Next\n\
         Distinct issues NOT already confirmed above, excluding near-duplicates from the \
         same underlying defect."
    };

    format!(
        "Contract: `{}`\nRounds completed: {}\n\nSource code:\n```solidity\n{}\n```\n\n\
         Confirmed bugs ({} finding{}):\n{}{}{}\n\n{}",
        request.contract_name,
        request.rounds_completed,
        request.source_code,
        request.confirmed_bugs.len(),
        if request.confirmed_bugs.len() == 1 {
            ""
        } else {
            "s"
        },
        bugs_section,
        fuzz_section,
        previous_section,
        instruction,
    )
}

fn format_bugs(bugs: &[BugInfo]) -> String {
    // Deduplicate by invariant name, preserve order.
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for bug in bugs {
        if seen.insert(&bug.invariant_name) {
            let seq = if bug.call_sequence.is_empty() {
                String::new()
            } else {
                format!(
                    "\n  Call sequence:\n{}",
                    bug.call_sequence
                        .lines()
                        .map(|l| format!("    {l}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            };
            out.push(format!("- `{}`{}", bug.invariant_name, seq));
        }
    }
    out.join("\n")
}
