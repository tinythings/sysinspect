use crate::{intp::actproc::response::ActionResponse, SysinspectError};

pub trait EventProcessorCallback: std::fmt::Debug {
    /// Run a callback
    fn on_action_response(&mut self, ar: ActionResponse) -> Result<(), SysinspectError>;
}
