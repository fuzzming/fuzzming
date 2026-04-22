use anyhow::Result;
use async_trait::async_trait;

use crate::fuzzer::ports::inbound::FuzzerRunPort;
use crate::shared::ports::FuzzerEnginePort;
use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::fuzz_report::FuzzReport;

pub struct Fuzzer {
    pub use_case: Box<dyn FuzzerRunPort>,
}

impl Fuzzer {
    pub fn new(use_case: Box<dyn FuzzerRunPort>) -> Self {
        Self { use_case }
    }
}

#[async_trait]
impl FuzzerEnginePort for Fuzzer {
    async fn run(&self, signal: RoundSignal) -> Result<FuzzReport> {
        self.use_case.run(signal).await
    }
}
