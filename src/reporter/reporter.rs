use crate::reporter::ports::OutputPort;
use crate::shared::ports::{ReporterPort, ReporterReaderPort};
use crate::shared::responses::session_outcome::SessionOutcome;

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
