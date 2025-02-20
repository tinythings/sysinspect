use crate::minion::SysMinion;
use async_trait::async_trait;
use libsysinspect::{intp::actproc::response::ActionResponse, reactor::callback::EventProcessorCallback, SysinspectError};
use std::sync::Arc;

#[derive(Debug)]
pub struct ActionResponseCallback {
    minion: Arc<SysMinion>,
}

impl ActionResponseCallback {
    pub(crate) fn new(minion: Arc<SysMinion>) -> Self {
        Self { minion }
    }
}

#[async_trait]
impl EventProcessorCallback for ActionResponseCallback {
    async fn on_action_response(&mut self, ar: ActionResponse) -> Result<(), SysinspectError> {
        self.minion.clone().send_callback(ar).await
    }
}
