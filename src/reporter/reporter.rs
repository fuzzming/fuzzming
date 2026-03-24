use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::signals::{SessionOutcome, FuzzPaths};
use crate::interfaces::contexts::ReportArtifacts;
use crate::orchestrator::ports::ReporterPort;
use crate::reporter::ports::{ReporterReaderPort, OutputPort};

pub struct Reporter {
    pub reader: Box<dyn ReporterReaderPort>,
    pub output: Box<dyn OutputPort>,
}

impl Reporter {
    pub fn new(reader: Box<dyn ReporterReaderPort>, output: Box<dyn OutputPort>) -> Self {
        Self { reader, output }
    }
}

impl ReporterPort for Reporter {
    fn emit(&self, outcome: SessionOutcome) {
        todo!()
    }
}
