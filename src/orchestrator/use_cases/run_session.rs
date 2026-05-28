use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::{join_all, try_join_all};
use tracing::info;

use crate::orchestrator::ports::inbound::OrchestratorRunPort;
use crate::orchestrator::use_cases::{
    check_termination::check_termination, initialise_session::initialise_session,
    run_round::run_round,
};
use crate::shared::models::{CoverageContext, FuzzerConfigArtifact, SessionState};
use crate::shared::ports::{
    ExecutorPort, FuzzerEnginePort, LlmEnginePort, ReaderPort, ReporterPort, SecurityAnalysisPort,
    SecurityAnalysisRequest,
};
use crate::shared::requests::{round_signal::RoundSignal, session_request::SessionRequest};
use crate::shared::responses::{
    fuzz_report::{FuzzOutcome, FuzzReport},
    round_usage::RoundUsage,
    session_outcome::{SessionOutcome, TerminationReason},
    stage_event::{FuzzerRoundSummary, StageEvent, StageKind, StageStatus},
    termination_decision::TerminationDecision,
};

pub struct RunSessionUseCase {
    pub llm_engine: Box<dyn LlmEnginePort>,
    pub fuzzer_engine: Box<dyn FuzzerEnginePort>,
    pub executor: Box<dyn ExecutorPort>,
    pub reporter: Box<dyn ReporterPort>,
    pub reader: Box<dyn ReaderPort>,
    pub security_analyzer: Option<Box<dyn SecurityAnalysisPort>>,
}

impl RunSessionUseCase {
    pub fn new(
        llm_engine: Box<dyn LlmEnginePort>,
        fuzzer_engine: Box<dyn FuzzerEnginePort>,
        executor: Box<dyn ExecutorPort>,
        reporter: Box<dyn ReporterPort>,
        reader: Box<dyn ReaderPort>,
    ) -> Self {
        Self {
            llm_engine,
            fuzzer_engine,
            executor,
            reporter,
            reader,
            security_analyzer: None,
        }
    }

    pub fn with_security_analyzer(mut self, sa: Box<dyn SecurityAnalysisPort>) -> Self {
        self.security_analyzer = Some(sa);
        self
    }
}

#[async_trait]
impl OrchestratorRunPort for RunSessionUseCase {
    async fn run(&self, request: SessionRequest) -> Result<Vec<SessionOutcome>> {
        let mut state = initialise_session(&request)?;
        let mut active: Vec<String> = request.target_paths.clone();
        let mut outcomes: Vec<SessionOutcome> = Vec::new();

        info!(
            contracts = active.len(),
            max_rounds = state.rounds_remaining,
            "session started"
        );

        loop {
            state.current_round += 1;
            info!(
                round = state.current_round,
                contracts = active.len(),
                "round started"
            );

            let mut signals: Vec<RoundSignal> =
                try_join_all(active.iter().map(|path| self.build_signal(path, &state))).await?;

            // Inject any LLM parse failure from the previous round so the model can self-correct.
            for signal in &mut signals {
                if let Some(err) = state.llm_failures.remove(&signal.contract_name) {
                    let prev = signal.fuzz_output.take().unwrap_or_default();
                    signal.fuzz_output = Some(format!(
                        "LLM PARSE FAILURE — your previous response could not be parsed. \
                         Fix your output format and try again.\nError: {err}\n\n\
                         Previous fuzz output (if any):\n{prev}"
                    ));
                }
            }

            // Security analysis — runs before generation for patch rounds (existing_bodies is Some).
            // Round 1 already has a 3-stage analysis built into the generator; skip it there.
            // Also skip when the previous round produced a compile/setup/LLM error: the model
            // should focus on fixing the error, not on new vulnerability suggestions.
            if let Some(sa) = &self.security_analyzer {
                let patch_indices: Vec<usize> = signals
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        let is_patch = s.existing_bodies.is_some();
                        let has_error = s
                            .fuzz_output
                            .as_deref()
                            .map(|o| {
                                o.contains("COMPILATION ERROR")
                                    || o.contains("SETUP FAILURE")
                                    || o.contains("LLM PARSE FAILURE")
                            })
                            .unwrap_or(false);
                        (is_patch && !has_error).then_some(i)
                    })
                    .collect();

                let reqs: Vec<SecurityAnalysisRequest> = patch_indices
                    .iter()
                    .map(|&i| SecurityAnalysisRequest {
                        contract_name: signals[i].contract_name.clone(),
                        source_code: signals[i].source_code.clone(),
                        confirmed_bugs: signals[i].confirmed_bugs.clone(),
                        fuzz_output: signals[i].fuzz_output.clone(),
                        rounds_completed: state.current_round.saturating_sub(1),
                        previous_analysis: state
                            .security_analyses
                            .get(&signals[i].contract_name)
                            .cloned(),
                    })
                    .collect();

                let results = join_all(reqs.into_iter().map(|req| sa.analyze(req))).await;

                for (&i, result) in patch_indices.iter().zip(results.into_iter()) {
                    match result {
                        Ok(analysis) => {
                            state
                                .security_analyses
                                .insert(signals[i].contract_name.clone(), analysis.clone());
                            signals[i].security_analysis = Some(analysis);
                        }
                        Err(e) => tracing::warn!(
                            contract = %signals[i].contract_name,
                            error = %e,
                            "security analysis failed"
                        ),
                    }
                }
            }

            let llm_signals = try_join_all(signals.iter().map(|signal| {
                run_round(
                    signal.clone(),
                    self.llm_engine.as_ref(),
                    self.executor.as_ref(),
                    self.reporter.as_ref(),
                )
            }))
            .await?;

            for (signal, llm_signal) in signals.iter().zip(llm_signals.iter()) {
                if let Some(result) = llm_signal.result.as_ref() {
                    let usage = RoundUsage {
                        contract_name: signal.contract_name.clone(),
                        round: state.current_round,
                        usage: result.usage.clone(),
                    };
                    self.reporter.emit_round_usage(usage).await?;
                }
                // Record parse failures for injection into the next round.
                if matches!(
                    llm_signal.status,
                    crate::shared::responses::llm_signal::LlmStatus::Failed
                ) {
                    if let Some(reason) = &llm_signal.reason {
                        state
                            .llm_failures
                            .insert(signal.contract_name.clone(), reason.clone());
                    }
                }
            }

            // Build a set of contracts whose LLM call failed this round.
            // For those contracts the executor wrote nothing new — running
            // forge would fuzz stale files from a previous session and report
            // the same old bugs again, polluting the deduplication map.
            let llm_failed_contracts: std::collections::HashSet<String> = signals
                .iter()
                .zip(llm_signals.iter())
                .filter(|(_, ls)| {
                    matches!(ls.status, crate::shared::responses::llm_signal::LlmStatus::Failed)
                })
                .map(|(s, _)| s.contract_name.clone())
                .collect();

            // Only fuzz contracts where the LLM succeeded.
            let fuzzable_signals: Vec<RoundSignal> = signals
                .iter()
                .filter(|s| !llm_failed_contracts.contains(&s.contract_name))
                .cloned()
                .collect();

            self.reporter
                .emit_stage_event(StageEvent {
                    contract_name: None,
                    round: state.current_round,
                    stage: StageKind::Fuzzer,
                    status: StageStatus::Started,
                    fuzzer_summary: None,
                })
                .await?;
            info!(round = state.current_round, "forge run started");
            let fuzz_reports: Vec<FuzzReport> = if fuzzable_signals.is_empty() {
                vec![]
            } else {
                self.fuzzer_engine.run(fuzzable_signals.clone()).await?
            };
            info!(round = state.current_round, "forge run finished");

            // Re-expand to the full signals list: failed contracts get a
            // CompileError placeholder so downstream logic stays consistent.
            let mut fuzz_iter = fuzz_reports.into_iter();
            let reports: Vec<FuzzReport> = signals
                .iter()
                .map(|s| {
                    if llm_failed_contracts.contains(&s.contract_name) {
                        FuzzReport {
                            outcome: FuzzOutcome::CompileError,
                            bugs: vec![],
                            lcov_path: None,
                        }
                    } else {
                        fuzz_iter.next().unwrap_or(FuzzReport {
                            outcome: FuzzOutcome::CompileError,
                            bugs: vec![],
                            lcov_path: None,
                        })
                    }
                })
                .collect();
            let fuzzer_summary = FuzzerRoundSummary {
                bugs: reports.iter().filter(|r| !r.bugs.is_empty()).count(),
                passed: reports
                    .iter()
                    .filter(|r| matches!(r.outcome, FuzzOutcome::Pass))
                    .count(),
                compile_errors: reports
                    .iter()
                    .filter(|r| matches!(r.outcome, FuzzOutcome::CompileError))
                    .count(),
            };
            self.reporter
                .emit_stage_event(StageEvent {
                    contract_name: None,
                    round: state.current_round,
                    stage: StageKind::Fuzzer,
                    status: StageStatus::Finished,
                    fuzzer_summary: Some(fuzzer_summary),
                })
                .await?;

            state.rounds_remaining = state.rounds_remaining.saturating_sub(1);

            let mut next_active: Vec<String> = Vec::new();
            let mut compile_error_emitted = false;

            for (((path, signal), report), llm_signal) in active
                .iter()
                .zip(signals.iter())
                .zip(reports.iter())
                .zip(llm_signals.iter())
            {
                // Accumulate bugs found this round — one entry per unique invariant name.
                // The same invariant can fire in multiple rounds; only keep the first
                // occurrence so the final report doesn't repeat the same finding N times.
                // Invariant code is read from final_bodies (the bodies forge just executed)
                // so the full Solidity function is preserved in the report.
                if !report.bugs.is_empty() {
                    let entry = state
                        .found_bugs
                        .entry(signal.contract_name.clone())
                        .or_default();
                    for bug in &report.bugs {
                        if !entry.iter().any(|b| b.invariant_name == bug.invariant_name) {
                            let mut bug_with_code = bug.clone();
                            if bug_with_code.invariant_code.is_empty() {
                                if let Some(bodies) = &llm_signal.final_bodies {
                                    if let Some(code) = bodies
                                        .invariant_test
                                        .invariants
                                        .get(&bug.invariant_name)
                                    {
                                        bug_with_code.invariant_code = code.clone();
                                    }
                                }
                            }
                            entry.push(bug_with_code);
                        }
                    }
                } else if let Some(lcov_path) = &report.lcov_path {
                    let lcov_str = lcov_path.to_string_lossy().to_string();
                    if let Ok(Some(ctx)) = self.reader.get_coverage_context(&lcov_str).await {
                        state
                            .coverage_snapshots
                            .entry(signal.contract_name.clone())
                            .or_default()
                            .push(format_coverage_summary(ctx));
                    }
                }

                // Emit compile errors once per round to avoid duplicate output.
                // Skip LLM-failed contracts — their CompileError is a placeholder;
                // reading fuzz_output.txt would show stale output from a previous session.
                if matches!(report.outcome, FuzzOutcome::CompileError)
                    && !compile_error_emitted
                    && !llm_failed_contracts.contains(&signal.contract_name)
                {
                    let fuzz_output_path =
                        format!(".fuzzming/{}/fuzz_output.txt", signal.contract_name);
                    if let Ok(Some(msg)) = self.reader.get_fuzz_output(&fuzz_output_path).await {
                        self.reporter
                            .emit_compile_error(state.current_round, &msg)
                            .await?;
                        compile_error_emitted = true;
                    }
                }

                let decision = check_termination(report, &state);
                let decision = if !decision.terminate {
                    self.check_full_coverage_streak(&signal.contract_name, report, &mut state)
                        .await?
                        .map(|reason| TerminationDecision {
                            terminate: true,
                            reason: Some(reason),
                        })
                        .unwrap_or(decision)
                } else {
                    decision
                };

                if decision.terminate {
                    let reason = decision.reason.ok_or_else(|| {
                        anyhow!(
                            "terminate=true but no reason for '{}'",
                            signal.contract_name
                        )
                    })?;
                    let all_bugs = state
                        .found_bugs
                        .get(&signal.contract_name)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    info!(
                        contract = %signal.contract_name,
                        reason = ?reason,
                        total_bugs = all_bugs.len(),
                        rounds = state.current_round,
                        "contract session terminated"
                    );
                    let outcome = SessionOutcome {
                        reason,
                        contract_name: signal.contract_name.clone(),
                        rounds_completed: state.current_round,
                        bugs: all_bugs.to_vec(),
                        coverage_snapshots: state
                            .coverage_snapshots
                            .remove(&signal.contract_name)
                            .unwrap_or_default(),
                        security_analysis: state
                            .security_analyses
                            .get(&signal.contract_name)
                            .cloned(),
                    };
                    let outcome_path = state
                        .config
                        .workspace_root
                        .join(format!(".fuzzming/{}/outcome.json", signal.contract_name));
                    if let Some(parent) = outcome_path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    let json = serde_json::to_string_pretty(&outcome)?;
                    tokio::fs::write(&outcome_path, json).await?;

                    let contract_done_status = if outcome.bugs.is_empty()
                        && !matches!(
                            outcome.reason,
                            TerminationReason::Bug
                                | TerminationReason::DevTestFailed
                                | TerminationReason::CompileError
                        ) {
                        StageStatus::Finished
                    } else {
                        StageStatus::Failed
                    };
                    self.reporter
                        .emit_stage_event(StageEvent {
                            contract_name: Some(signal.contract_name.clone()),
                            round: state.current_round,
                            stage: StageKind::ContractDone,
                            status: contract_done_status,
                            fuzzer_summary: None,
                        })
                        .await?;

                    outcomes.push(outcome);
                } else {
                    let bug_count = state
                        .found_bugs
                        .get(&signal.contract_name)
                        .map(Vec::len)
                        .unwrap_or(0);
                    info!(
                        contract = %signal.contract_name,
                        outcome = ?report.outcome,
                        bugs_so_far = bug_count,
                        rounds_remaining = state.rounds_remaining,
                        "round complete — continuing"
                    );
                    next_active.push(path.clone());
                }
            }

            active = next_active;

            if active.is_empty() {
                break;
            }
        }

        if outcomes.is_empty() {
            return Err(anyhow!("session produced no outcome"));
        }

        Ok(outcomes)
    }
}

impl RunSessionUseCase {
    async fn build_signal(&self, contract_path: &str, state: &SessionState) -> Result<RoundSignal> {
        let contract_name = extract_contract_name(contract_path);
        let fuzz_output_path = format!(".fuzzming/{contract_name}/fuzz_output.txt");
        let lcov_path = format!(".fuzzming/{contract_name}/coverage_context.json");
        let bodies_path = format!(".fuzzming/{contract_name}/{contract_name}.bodies.json");
        let config_path = format!(".fuzzming/{contract_name}/{contract_name}.config.json");

        let (contract_context, fuzz_output, coverage_context, existing_bodies, existing_config) = tokio::try_join!(
            self.reader.get_contract_context(contract_path, false),
            self.reader.get_fuzz_output(&fuzz_output_path),
            self.reader.get_coverage_context(&lcov_path),
            self.reader.get_existing_bodies(&bodies_path),
            self.reader.get_existing_config(&config_path),
        )?;

        let existing_foundry_config = existing_config.map(|c| match c {
            FuzzerConfigArtifact::Foundry(fc) => fc,
        });

        let confirmed_bugs = state
            .found_bugs
            .get(&contract_name)
            .cloned()
            .unwrap_or_default();

        let source_code = contract_context.source_code;
        let source_pragma = extract_pragma_from_source(&source_code);

        Ok(RoundSignal {
            round: state.current_round,
            config: state.config.clone(),
            contract_name,
            contract_path: contract_path.to_string(),
            source_code,
            source_pragma,
            fuzz_output,
            coverage_context,
            existing_bodies,
            existing_foundry_config,
            confirmed_bugs,
            security_analysis: None,
        })
    }

    async fn check_full_coverage_streak(
        &self,
        contract_name: &str,
        report: &FuzzReport,
        state: &mut SessionState,
    ) -> Result<Option<TerminationReason>> {
        if !matches!(report.outcome, FuzzOutcome::Pass) {
            state.full_coverage_streak.remove(contract_name);
            return Ok(None);
        }

        let lcov_path = match &report.lcov_path {
            Some(p) => p.to_string_lossy().to_string(),
            None => return Ok(None),
        };

        let ctx = match self.reader.get_coverage_context(&lcov_path).await? {
            Some(ctx) => ctx,
            None => return Ok(None),
        };

        let full = ctx.line_found > 0
            && ctx.line_hit == ctx.line_found
            && (ctx.branch_found == 0 || ctx.branch_hit == ctx.branch_found)
            && (ctx.function_found == 0 || ctx.function_hit == ctx.function_found);

        if full {
            let streak = state
                .full_coverage_streak
                .entry(contract_name.to_string())
                .or_insert(0);
            *streak += 1;
            info!(contract = %contract_name, streak = *streak, threshold = state.config.full_coverage_rounds, "full coverage streak");
            if *streak >= state.config.full_coverage_rounds {
                return Ok(Some(TerminationReason::FullCoverage));
            }
        } else {
            state.full_coverage_streak.remove(contract_name);
        }

        Ok(None)
    }
}

fn extract_contract_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn format_coverage_summary(ctx: CoverageContext) -> String {
    format!(
        "Lines:     {}/{}\nBranches:  {}/{}\nFunctions: {}/{}\nUncovered gaps: {}",
        ctx.line_hit,
        ctx.line_found,
        ctx.branch_hit,
        ctx.branch_found,
        ctx.function_hit,
        ctx.function_found,
        ctx.gaps.len(),
    )
}

fn extract_pragma_from_source(source: &str) -> String {
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
    "^0.8.20".to_string()
}
