use std::collections::HashMap;

use anyhow::Result;

use crate::shared::{models::SessionState, requests::session_request::SessionRequest};

pub fn initialise_session(request: &SessionRequest) -> Result<SessionState> {
    Ok(SessionState {
        rounds_remaining: request.max_rounds,
        current_round: 0,
        config: request.config.clone(),
        found_bugs: HashMap::new(),
        full_coverage_streak: HashMap::new(),
        coverage_snapshots: HashMap::new(),
        llm_failures: HashMap::new(),
        security_analyses: HashMap::new(),
    })
}
