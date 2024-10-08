use crate::intp::actproc::response::ActionResponse;

pub trait EventHandler {
    fn id(&self) -> String;
    fn handle(&self, evt: &ActionResponse);
}
