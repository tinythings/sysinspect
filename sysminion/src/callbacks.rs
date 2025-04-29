use crate::minion::SysMinion;
use async_trait::async_trait;
use libsysinspect::{SysinspectError, intp::actproc::response::ActionResponse, reactor::callback::EventProcessorCallback};
use std::sync::Arc;

// Callback for action response fired after each action/event.
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

/// Callback for model response at the end of the model cycle.
#[derive(Debug)]
pub struct ModelResponseCallback {
    minion: Arc<SysMinion>,
    cid: String,
}

impl ModelResponseCallback {
    pub(crate) fn new(minion: Arc<SysMinion>, cid: &str) -> Self {
        Self { minion, cid: cid.to_owned() }
    }
}

#[async_trait]
impl EventProcessorCallback for ModelResponseCallback {
    async fn on_action_response(&mut self, mut ar: ActionResponse) -> Result<(), SysinspectError> {
        ar.set_cid(self.cid.to_owned());
        self.minion.clone().send_fin_callback(ar).await
    }
}
