use anyhow::Result;
use async_trait::async_trait;
use crate::interfaces::signals::{RoundSignal, FuzzReport};

#[async_trait]
pub trait FuzzerEnginePort: Send + Sync {
    async fn run(&self, signal: RoundSignal) -> Result<FuzzReport>;
}
