use crate::shared::models::ReportArtifacts;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ReporterReaderPort: Send + Sync {
    async fn get_report_artifacts(&self, contract_name: &str) -> Result<ReportArtifacts>;
}
