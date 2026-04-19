use anyhow::Result;
use crate::interfaces::signals::{SessionRequest, SessionOutcome};
use crate::interfaces::ports::{LlmEnginePort, FuzzerEnginePort, ReporterPort, ExecutorPort};

pub struct SessionOrchestrator {
    pub llm_engine: Box<dyn LlmEnginePort>,
    pub fuzzer_engine: Box<dyn FuzzerEnginePort>,
    pub executor: Box<dyn ExecutorPort>,
    pub reporter: Box<dyn ReporterPort>,
}

impl SessionOrchestrator {
    pub fn new(
        llm_engine: Box<dyn LlmEnginePort>,
        fuzzer_engine: Box<dyn FuzzerEnginePort>,
        executor: Box<dyn ExecutorPort>,
        reporter: Box<dyn ReporterPort>,
    ) -> Self {
        Self { llm_engine, fuzzer_engine, executor, reporter }
    }

    pub async fn run(&self, request: SessionRequest) -> Result<SessionOutcome> {
        todo!()
    }
}
