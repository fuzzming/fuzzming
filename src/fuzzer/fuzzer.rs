use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::signals::{RoundSignal, FuzzReport};
use crate::interfaces::ports::FuzzerEnginePort;
use crate::fuzzer::ports::TestRunnerPort;

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
