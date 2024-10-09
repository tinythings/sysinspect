use crate::intp::{actproc::response::ActionResponse, conf::EventConfig};
use std::fmt::Debug;

pub trait EventHandler: Debug {
    /// Constructor
    fn new(eid: String, cfg: EventConfig) -> Self
    where
        Self: Sized;

    /// Returns Id of the handler
    fn id() -> String
    where
        Self: Sized;

    /// Calls the handler on the specific action
    fn handle(&self, evt: &ActionResponse);
}
