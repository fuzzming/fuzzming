use anyhow::Result;
use async_trait::async_trait;

use crate::reporter::ports::outbound::OutputPort;
use crate::reporter::use_cases::{
    format_bug_report, format_compile_error, format_coverage_report, format_dev_test_failure,
    format_exhausted_report, format_round_usage,
};
use crate::shared::ports::ReporterPort;
use crate::shared::responses::{
    round_usage::RoundUsage,
    session_outcome::{SessionOutcome, TerminationReason},
    stage_event::StageEvent,
};

pub struct Reporter {
    pub output: Box<dyn OutputPort>,
}

impl Reporter {
    pub fn new(output: Box<dyn OutputPort>) -> Self {
        Self { output }
    }
}

#[async_trait]
impl ReporterPort for Reporter {
    async fn emit(&self, outcome: SessionOutcome) -> Result<()> {
        let message = match outcome.reason {
            TerminationReason::Bug => format_bug_report(&outcome),
            TerminationReason::FullCoverage => format_coverage_report(&outcome),
            TerminationReason::DevTestFailed => format_dev_test_failure(&outcome),
            TerminationReason::Exhausted => format_exhausted_report(&outcome),
        };

        self.output.write(&message).await
    }

    async fn emit_round_usage(&self, usage: RoundUsage) -> Result<()> {
        let message = format_round_usage(&usage);
        self.output.write_progress(&message).await
    }

    async fn emit_stage_event(&self, event: StageEvent) -> Result<()> {
        self.output.handle_stage_event(event).await
    }

    async fn emit_compile_error(&self, round: u32, message: &str) -> Result<()> {
        let formatted = format_compile_error(round, message);
        self.output.write(&formatted).await
    }
}
