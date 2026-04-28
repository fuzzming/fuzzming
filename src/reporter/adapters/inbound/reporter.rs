use anyhow::Result;
use async_trait::async_trait;

use crate::reporter::ports::outbound::OutputPort;
use crate::reporter::use_cases::{
    format_bug_report, format_coverage_report, format_dev_test_failure, format_exhausted_report,
};
use crate::shared::ports::{ReporterPort, ReporterReaderPort};
use crate::shared::responses::session_outcome::{SessionOutcome, TerminationReason};

pub struct Reporter {
    pub reader: Box<dyn ReporterReaderPort>,
    pub output: Box<dyn OutputPort>,
}

impl Reporter {
    pub fn new(reader: Box<dyn ReporterReaderPort>, output: Box<dyn OutputPort>) -> Self {
        Self { reader, output }
    }
}

#[async_trait]
impl ReporterPort for Reporter {
    async fn emit(&self, outcome: SessionOutcome) -> Result<()> {
        let mut artifacts = self
            .reader
            .get_report_artifacts(&outcome.contract_name)
            .await?;
        artifacts.round_history = outcome.rounds_completed;

        let message = match outcome.reason {
            TerminationReason::Bug => format_bug_report(&artifacts),
            TerminationReason::FullCoverage => format_coverage_report(&artifacts),
            TerminationReason::DevTestFailed => format_dev_test_failure(&artifacts),
            TerminationReason::Exhausted => format_exhausted_report(&artifacts),
        };

        self.output.write(&message).await
    }
}
