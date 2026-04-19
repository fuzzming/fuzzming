use crate::orchestrator::domain::Session;
use crate::shared::requests::session_request::SessionRequest;

pub fn initialise_session(request: &SessionRequest) -> Session {
    Session::new(request)
}
