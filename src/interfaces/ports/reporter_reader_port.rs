use crate::interfaces::contexts::ReportArtifacts;
use crate::interfaces::signals::FuzzPaths;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ReporterReaderPort: Send + Sync {
    async fn get_report_artifacts(&self, paths: &FuzzPaths) -> Result<ReportArtifacts>;
}
