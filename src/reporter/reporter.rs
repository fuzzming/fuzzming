use crate::interfaces::signals::SessionOutcome;
use crate::interfaces::ports::{ReporterPort, ReporterReaderPort};
use crate::reporter::ports::OutputPort;

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
