use crate::shared::{requests::round_signal::RoundSignal, responses::fuzz_report::FuzzReport};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait FuzzerEnginePort: Send + Sync {
    async fn run(&self, signals: Vec<RoundSignal>) -> Result<Vec<FuzzReport>>;
}
