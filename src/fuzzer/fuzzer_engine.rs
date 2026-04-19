use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::signals::{RoundSignal, FuzzReport};
use crate::interfaces::ports::{FuzzerEnginePort, FuzzerReaderPort};
use crate::fuzzer::ports::TestRunnerPort;

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
