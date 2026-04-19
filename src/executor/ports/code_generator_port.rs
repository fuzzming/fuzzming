use crate::executor::infrastructure::FileSystemWriter;
use crate::interfaces::artifacts::BodiesJson;
use anyhow::Result;
use async_trait::async_trait;

/// Language axis — generates source test files from a BodiesJson artifact.
/// One implementation per supported language.
/// The Executor holds this as Arc<dyn CodeGeneratorPort> injected at composition time.
#[async_trait]
pub trait CodeGeneratorPort: Send + Sync {
    async fn generate(&self, bodies: &BodiesJson, writer: &FileSystemWriter) -> Result<()>;
}
