use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::signals::{RoundSignal, FuzzReport};
use crate::fuzzer::ports::{FuzzerReaderPort, TestRunnerPort};
use crate::orchestrator::ports::FuzzerEnginePort;

pub struct FuzzerEngine {
    pub reader: Box<dyn FuzzerReaderPort>,
    pub runner: Box<dyn TestRunnerPort>,
}

impl FuzzerEngine {
    pub fn new(
        reader: Box<dyn FuzzerReaderPort>,
        runner: Box<dyn TestRunnerPort>,
    ) -> Self {
        Self { reader, runner }
    }
}

#[async_trait]
impl FuzzerEnginePort for FuzzerEngine {
    async fn run(&self, signal: RoundSignal) -> Result<FuzzReport> {
        todo!()
    }
}
