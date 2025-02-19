use std::sync::Arc;

use libsysinspect::{intp::actproc::response::ActionResponse, reactor::callback::EventProcessorCallback, SysinspectError};
use tokio::runtime::Handle;

use crate::minion::SysMinion;

#[derive(Debug)]
pub struct ActionResponseCallback {
    minion: Arc<SysMinion>,
}

impl ActionResponseCallback {
    pub(crate) fn new(minion: Arc<SysMinion>) -> Self {
        Self { minion }
    }
}

impl EventProcessorCallback for ActionResponseCallback {
    fn on_action_response(&mut self, ar: ActionResponse) -> Result<(), SysinspectError> {
        self.minion.clone().send_callback(ar);

        Ok(())
    }
}
