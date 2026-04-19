use crate::fuzzer::ports::TestRunnerPort;
use crate::shared::ports::FuzzerEnginePort;
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::fuzz_report::FuzzReport;
use anyhow::Result;
use async_trait::async_trait;

pub struct Fuzzer {
    pub runner: Box<dyn TestRunnerPort>,
}

impl Fuzzer {
    pub fn new(runner: Box<dyn TestRunnerPort>) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl FuzzerEnginePort for Fuzzer {
    async fn run(&self, signal: RoundSignal) -> Result<FuzzReport> {
        todo!()
    }
}
