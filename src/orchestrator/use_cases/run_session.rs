use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::try_join_all;
use tracing::info;

use crate::orchestrator::ports::inbound::OrchestratorRunPort;
use crate::orchestrator::use_cases::{
    check_termination::check_termination, initialise_session::initialise_session,
    run_round::run_round,
};
use crate::shared::models::{BugInfo, CoverageContext, FuzzerConfigArtifact, ReportArtifacts, SessionState};
use crate::shared::ports::{ExecutorPort, FuzzerEnginePort, LlmEnginePort, ReaderPort, ReporterPort};
use crate::shared::requests::{round_signal::RoundSignal, session_request::SessionRequest};
use crate::shared::responses::{
    fuzz_report::FuzzReport,
    round_usage::RoundUsage,
    session_outcome::{SessionOutcome, TerminationReason},
    stage_event::{StageEvent, StageKind, StageStatus},
};

pub struct RunSessionUseCase {
    pub llm_engine: Box<dyn LlmEnginePort>,
    pub fuzzer_engine: Box<dyn FuzzerEnginePort>,
    pub executor: Box<dyn ExecutorPort>,
    pub reporter: Box<dyn ReporterPort>,
    pub reader: Box<dyn ReaderPort>,
}

impl RunSessionUseCase {
    pub fn new(
        llm_engine: Box<dyn LlmEnginePort>,
        fuzzer_engine: Box<dyn FuzzerEnginePort>,
        executor: Box<dyn ExecutorPort>,
        reporter: Box<dyn ReporterPort>,
        reader: Box<dyn ReaderPort>,
    ) -> Self {
        Self { llm_engine, fuzzer_engine, executor, reporter, reader }
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
            info!(round = state.current_round, contracts = active.len(), "round started");

            // 1. Read context for all active contracts in parallel.
            let signals: Vec<RoundSignal> = try_join_all(
                active.iter().map(|path| self.build_signal(path, &state)),
            )
            .await?;

            // 2. LLM + Executor for all contracts in parallel.
            let llm_signals = try_join_all(
                signals.iter().map(|signal| {
                    run_round(
                        signal.clone(),
                        self.llm_engine.as_ref(),
                        self.executor.as_ref(),
                        self.reporter.as_ref(),
                    )
                }),
            )
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
            }

            // 3. One forge run for all contracts.
            self.reporter
                .emit_stage_event(StageEvent {
                    contract_name: None,
                    round: state.current_round,
                    stage: StageKind::Fuzzer,
                    status: StageStatus::Started,
                })
                .await?;
            info!(round = state.current_round, "forge run started");
            let reports: Vec<FuzzReport> = self.fuzzer_engine.run(signals.clone()).await?;
            info!(round = state.current_round, "forge run finished");
            self.reporter
                .emit_stage_event(StageEvent {
                    contract_name: None,
                    round: state.current_round,
                    stage: StageKind::Fuzzer,
                    status: StageStatus::Finished,
                })
                .await?;

            // Decrement before termination check so the last round triggers Exhausted on Pass.
            state.rounds_remaining = state.rounds_remaining.saturating_sub(1);

            // 4. Accumulate bugs, check termination, emit reports for contracts that are done.
            let mut next_active: Vec<String> = Vec::new();

            for ((path, signal), report) in active.iter().zip(signals.iter()).zip(reports.iter()) {
                // Accumulate all bugs found this round before deciding termination.
                if !report.bugs.is_empty() {
                    state
                        .found_bugs
                        .entry(signal.contract_name.clone())
                        .or_default()
                        .extend(report.bugs.iter().cloned());
                }

                let decision = check_termination(report, &state);

                if decision.terminate {
                    let reason = decision.reason.ok_or_else(|| {
                        anyhow!("terminate=true but no reason for '{}'", signal.contract_name)
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
                    let artifacts =
                        self.read_artifacts(&signal.contract_name, all_bugs, report, &reason)
                            .await?;
                    let outcome = SessionOutcome {
                        reason,
                        contract_name: signal.contract_name.clone(),
                        rounds_completed: state.current_round,
                        bugs: all_bugs.to_vec(),
                        artifacts,
                    };
                    self.reporter.emit(outcome.clone()).await?;
                    let outcome_path = state.config.workspace_root
                        .join(format!(".fuzzming/{}/outcome.json", signal.contract_name));
                    let json = serde_json::to_string_pretty(&outcome)?;
                    tokio::fs::write(&outcome_path, json).await?;
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
            Err(anyhow!("session produced no outcome"))
        } else {
            Ok(outcomes)
        }
    }
}

impl RunSessionUseCase {
    async fn build_signal(&self, contract_path: &str, state: &SessionState) -> Result<RoundSignal> {
        let contract_name = extract_contract_name(contract_path);
        let fuzz_output_path = format!(".fuzzming/{}/fuzz_output.txt", contract_name);
        let lcov_path = format!(".fuzzming/{}/lcov.info", contract_name);
        let bodies_path = format!(".fuzzming/{}/{}.bodies.json", contract_name, contract_name);
        let config_path = format!(".fuzzming/{}/{}.config.json", contract_name, contract_name);

        let (contract_context, fuzz_output, coverage_context, existing_bodies, existing_config) =
            tokio::try_join!(
                self.reader.get_contract_context(contract_path, false),
                self.reader.get_fuzz_output(&fuzz_output_path),
                self.reader.get_coverage_context(&lcov_path),
                self.reader.get_existing_bodies(&bodies_path),
                self.reader.get_existing_config(&config_path),
            )?;

        let existing_foundry_config = existing_config.and_then(|c| match c {
            FuzzerConfigArtifact::Foundry(fc) => Some(fc),
        });

        let confirmed_bugs =
            state.found_bugs.get(&contract_name).cloned().unwrap_or_default();

        Ok(RoundSignal {
            round: state.current_round,
            config: state.config.clone(),
            contract_name,
            contract_path: contract_path.to_string(),
            source_code: contract_context.source_code,
            fuzz_output,
            coverage_context,
            existing_bodies,
            existing_foundry_config,
            confirmed_bugs,
        })
    }

    async fn read_artifacts(
        &self,
        contract_name: &str,
        all_bugs: &[BugInfo],
        _report: &FuzzReport,
        reason: &TerminationReason,
    ) -> Result<ReportArtifacts> {
        let fuzz_output_path = format!(".fuzzming/{}/fuzz_output.txt", contract_name);
        let lcov_path = format!(".fuzzming/{}/lcov.info", contract_name);
        let fuzz_output =
            self.reader.get_fuzz_output(&fuzz_output_path).await?.unwrap_or_default();

        let coverage_summary = match reason {
            TerminationReason::FullCoverage | TerminationReason::Exhausted => {
                self.reader
                    .get_coverage_context(&lcov_path)
                    .await?
                    .map(format_coverage_summary)
            }
            _ => None,
        };

        // All bugs accumulated across every round, not just the last one.
        let call_sequences = all_bugs
            .iter()
            .map(|b| format!("{}:\n{}", b.invariant_name, b.call_sequence))
            .collect();

        Ok(ReportArtifacts { fuzz_output, coverage_summary, call_sequences })
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
