use anyhow::Result;
use crate::interfaces::signals::{SessionRequest, SessionOutcome};
use crate::orchestrator::ports::{LlmEnginePort, FuzzerEnginePort, ReporterPort};

pub struct SessionOrchestrator {
    pub llm_engine: Box<dyn LlmEnginePort>,
    pub fuzzer_engine: Box<dyn FuzzerEnginePort>,
    pub reporter: Box<dyn ReporterPort>,
}

impl SessionOrchestrator {
    pub fn new(
        llm_engine: Box<dyn LlmEnginePort>,
        fuzzer_engine: Box<dyn FuzzerEnginePort>,
        reporter: Box<dyn ReporterPort>,
    ) -> Self {
        Self {
            llm_engine,
            fuzzer_engine,
            reporter,
        }
    }

    pub async fn run(&self, request: SessionRequest) -> Result<SessionOutcome> {
        todo!()
    }
}
