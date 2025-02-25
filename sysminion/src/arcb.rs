use crate::minion::SysMinion;
use async_trait::async_trait;
use libsysinspect::{SysinspectError, intp::actproc::response::ActionResponse, reactor::callback::EventProcessorCallback};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug)]
pub struct ActionResponseCallback {
    cid: String,
    minion: Arc<SysMinion>,
}

impl ActionResponseCallback {
    pub(crate) fn new(minion: Arc<SysMinion>) -> Self {
        Self { minion, cid: Uuid::new_v4().to_string() }
    }
}

#[async_trait]
impl EventProcessorCallback for ActionResponseCallback {
    async fn on_action_response(&mut self, mut ar: ActionResponse) -> Result<(), SysinspectError> {
        ar.set_cid(self.cid.to_owned());
        self.minion.clone().send_callback(ar).await
    }
}
