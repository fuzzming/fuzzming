use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::try_join_all;

use crate::orchestrator::ports::inbound::OrchestratorRunPort;
use crate::orchestrator::use_cases::{
    check_termination::check_termination, initialise_session::initialise_session,
    run_round::run_round,
};
use crate::shared::models::{CoverageContext, ReportArtifacts, SessionState};
use crate::shared::ports::{ExecutorPort, FuzzerEnginePort, LlmEnginePort, ReaderPort, ReporterPort};
use crate::shared::requests::{round_signal::RoundSignal, session_request::SessionRequest};
use crate::shared::responses::{
    fuzz_report::FuzzReport,
    session_outcome::{SessionOutcome, TerminationReason},
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
    async fn run(&self, request: SessionRequest) -> Result<SessionOutcome> {
        let mut state = initialise_session(&request)?;
        let mut active: Vec<String> = request.target_paths.clone();
        let mut last_outcome: Option<SessionOutcome> = None;

        loop {
            state.current_round += 1;

            // 1. Read context for all active contracts in parallel.
            let signals: Vec<RoundSignal> = try_join_all(
                active.iter().map(|path| self.build_signal(path, &state)),
            )
            .await?;

            // 2. LLM + Executor for all contracts in parallel.
            try_join_all(
                signals.iter().map(|signal| run_round(signal.clone(), self.llm_engine.as_ref(), self.executor.as_ref())),
            )
            .await?;

            // 3. One forge run for all contracts.
            let reports: Vec<FuzzReport> = self.fuzzer_engine.run(signals.clone()).await?;

            // Decrement before termination check so the last round triggers Exhausted on Pass.
            state.rounds_remaining = state.rounds_remaining.saturating_sub(1);

            // 4. Check termination per contract; emit report for those that are done.
            let mut next_active: Vec<String> = Vec::new();

            for ((path, signal), report) in active.iter().zip(signals.iter()).zip(reports.iter()) {
                let decision = check_termination(report, &state);

                if decision.terminate {
                    let reason = decision
                        .reason
                        .ok_or_else(|| anyhow!("terminate=true but no reason for '{}'", signal.contract_name))?;
                    let artifacts = self.read_artifacts(&signal.contract_name, report, &reason).await?;
                    let outcome = SessionOutcome {
                        reason,
                        contract_name: signal.contract_name.clone(),
                        rounds_completed: state.current_round,
                        artifacts,
                    };
                    self.reporter.emit(outcome.clone()).await?;
                    last_outcome = Some(outcome);
                } else {
                    next_active.push(path.clone());
                }
            }

            active = next_active;

            if active.is_empty() {
                break;
            }
        }

        last_outcome.ok_or_else(|| anyhow!("session produced no outcome"))
    }
}

impl RunSessionUseCase {
    async fn build_signal(&self, contract_path: &str, state: &SessionState) -> Result<RoundSignal> {
        let contract_name = extract_contract_name(contract_path);
        let fuzz_output_path = format!(".fuzzming/{}/fuzz_output.txt", contract_name);
        let lcov_path = format!(".fuzzming/{}/lcov.info", contract_name);
        let bodies_path = format!(".fuzzming/{}/{}.bodies.json", contract_name, contract_name);

        let (contract_context, fuzz_output, coverage_context, existing_bodies) = tokio::try_join!(
            self.reader.get_contract_context(contract_path, false),
            self.reader.get_fuzz_output(&fuzz_output_path),
            self.reader.get_coverage_context(&lcov_path),
            self.reader.get_existing_bodies(&bodies_path),
        )?;

        Ok(RoundSignal {
            round: state.current_round,
            config: state.config.clone(),
            contract_name,
            contract_path: contract_path.to_string(),
            source_code: contract_context.source_code,
            fuzz_output,
            coverage_context,
            existing_bodies,
            existing_foundry_config: None,
        })
    }

    async fn read_artifacts(
        &self,
        contract_name: &str,
        report: &FuzzReport,
        reason: &TerminationReason,
    ) -> Result<ReportArtifacts> {
        let fuzz_output_path = format!(".fuzzming/{}/fuzz_output.txt", contract_name);
        let fuzz_output = self.reader.get_fuzz_output(&fuzz_output_path).await?.unwrap_or_default();

        let coverage_summary = match reason {
            TerminationReason::FullCoverage | TerminationReason::Exhausted => {
                if let Some(lcov_path) = &report.lcov_path {
                    self.reader
                        .get_coverage_context(&lcov_path.to_string_lossy())
                        .await?
                        .map(format_coverage_summary)
                } else {
                    None
                }
            }
            _ => None,
        };

        Ok(ReportArtifacts {
            fuzz_output,
            coverage_summary,
            call_sequences: vec![],
        })
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
