use crate::shared::responses::session_outcome::SessionOutcome;

pub trait ReporterPort: Send + Sync {
    fn emit(&self, outcome: SessionOutcome);
}
