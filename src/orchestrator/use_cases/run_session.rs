use anyhow::Result;
use async_trait::async_trait;

use crate::orchestrator::ports::inbound::OrchestratorRunPort;
use crate::shared::ports::{ExecutorPort, FuzzerEnginePort, LlmEnginePort, ReaderPort, ReporterPort};
use crate::shared::requests::session_request::SessionRequest;
use crate::shared::responses::session_outcome::SessionOutcome;

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
        Self {
            llm_engine,
            fuzzer_engine,
            executor,
            reporter,
            reader,
        }
    }
}

#[async_trait]
impl OrchestratorRunPort for RunSessionUseCase {
    async fn run(&self, _request: SessionRequest) -> Result<SessionOutcome> {
        todo!()
    }
}
