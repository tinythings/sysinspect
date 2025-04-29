use crate::minion::SysMinion;
use async_trait::async_trait;
use libsysinspect::{SysinspectError, intp::actproc::response::ActionResponse, reactor::callback::EventProcessorCallback};
use std::sync::Arc;

#[derive(Debug)]
pub struct ActionResponseCallback {
    cid: String,
    minion: Arc<SysMinion>,
}

impl ActionResponseCallback {
    /// The `cid` (Cycle ID) is used to identify the master cycle, so the response
    /// is registered with the other minions, grouped into the same call session.
    pub(crate) fn new(minion: Arc<SysMinion>, cid: &str) -> Self {
        Self { minion, cid: cid.to_owned() }
    }
}

#[async_trait]
impl EventProcessorCallback for ActionResponseCallback {
    async fn on_action_response(&mut self, mut ar: ActionResponse) -> Result<(), SysinspectError> {
        ar.set_cid(self.cid.to_owned());
        self.minion.clone().send_callback(ar).await
    }
}
