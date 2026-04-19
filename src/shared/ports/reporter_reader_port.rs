use crate::shared::{models::ReportArtifacts, responses::fuzz_report::FuzzPaths};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ReporterReaderPort: Send + Sync {
    async fn get_report_artifacts(&self, paths: &FuzzPaths) -> Result<ReportArtifacts>;
}
