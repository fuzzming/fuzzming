use anyhow::Result;
use async_trait::async_trait;

use crate::shared::requests::round_signal::RoundSignal;
use crate::shared::responses::fuzz_report::FuzzReport;

#[async_trait]
pub trait FuzzerRunPort: Send + Sync {
    async fn run(&self, signals: Vec<RoundSignal>) -> Result<Vec<FuzzReport>>;
}
