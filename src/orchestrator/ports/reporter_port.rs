use crate::interfaces::signals::SessionOutcome;

pub trait ReporterPort: Send + Sync {
    fn emit(&self, outcome: SessionOutcome);
}
