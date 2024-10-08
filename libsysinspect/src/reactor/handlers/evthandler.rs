use crate::intp::actproc::response::ActionResponse;

pub trait EventHandler {
    fn handle(&self, evt: &ActionResponse);
}
