use crate::shared::{models::SessionState, requests::session_request::SessionRequest};
use anyhow::Result;

pub fn initialise_session(request: &SessionRequest) -> Result<SessionState> {
    Ok(SessionState {
        rounds_remaining: request.max_rounds,
        current_round: 0,
        config: request.config.clone(),
    })
}
