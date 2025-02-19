use crate::minion::SysMinion;
use async_trait::async_trait;
use libsysinspect::{intp::actproc::response::ActionResponse, reactor::callback::AsyncEventProcessorCallback, SysinspectError};
use std::sync::Arc;

#[derive(Debug)]
pub struct AsyncActionResponseCallback {
    minion: Arc<SysMinion>,
}

impl AsyncActionResponseCallback {
    pub(crate) fn new(minion: Arc<SysMinion>) -> Self {
        Self { minion }
    }
}

#[async_trait]
impl AsyncEventProcessorCallback for AsyncActionResponseCallback {
    async fn on_action_response(&mut self, ar: ActionResponse) -> Result<(), SysinspectError> {
        self.minion.clone().send_callback(ar).await
    }
}
