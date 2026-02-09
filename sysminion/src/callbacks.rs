use crate::minion::SysMinion;
use async_trait::async_trait;
use libcommon::SysinspectError;
use libsysinspect::{
    intp::actproc::response::{ActionModResponse, ActionResponse, ConstraintResponse},
    mdescr::telemetry::TelemetrySpec,
    reactor::callback::EventProcessorCallback,
};
use std::sync::Arc;

// Callback for action response fired after each action/event.
#[derive(Debug)]
pub struct ActionResponseCallback {
    cid: String,
    minion: Arc<SysMinion>,
    telemetry_config: Option<TelemetrySpec>,
}

impl ActionResponseCallback {
    /// The `cid` (Cycle ID) is used to identify the master cycle, so the response
    /// is registered with the other minions, grouped into the same call session.
    pub(crate) fn new(minion: Arc<SysMinion>, cid: &str) -> Self {
        Self { minion, cid: cid.to_owned(), telemetry_config: None }
    }
}

#[async_trait]
impl EventProcessorCallback for ActionResponseCallback {
    async fn on_action_response(&mut self, mut ar: ActionResponse) -> Result<(), SysinspectError> {
        ar.set_cid(self.cid.to_owned());
        if let Some(tcfg) = &self.telemetry_config {
            ar.set_telemetry_config(tcfg.action());
        }
        self.minion.clone().send_callback(ar).await
    }

    fn set_telemetry_config(&mut self, _telemetry_config: Option<TelemetrySpec>) {
        self.telemetry_config = _telemetry_config;
    }
}

/// Callback for model response at the end of the model cycle.
#[derive(Debug)]
pub struct ModelResponseCallback {
    minion: Arc<SysMinion>,
    cid: String,
    telemetry_config: Option<TelemetrySpec>,
}

impl ModelResponseCallback {
    pub(crate) fn new(minion: Arc<SysMinion>, cid: &str) -> Self {
        Self { minion, cid: cid.to_owned(), telemetry_config: None }
    }
}

#[async_trait]
impl EventProcessorCallback for ModelResponseCallback {
    async fn on_action_response(&mut self, ar: ActionResponse) -> Result<(), SysinspectError> {
        // Reset the data in the final response,
        // because we need to carry only telemetry
        // configuration data.
        let mut fin = ActionResponse::new(
            ar.eid().to_owned(),
            ar.aid().to_owned(),
            ar.sid().to_owned(),
            ActionModResponse::default(),
            ConstraintResponse::default(),
        );

        fin.set_cid(self.cid.to_owned());
        if let Some(tcfg) = &self.telemetry_config {
            fin.set_telemetry_config(tcfg.minion());
        }

        self.minion.clone().send_fin_callback(fin).await
    }

    fn set_telemetry_config(&mut self, _telemetry_config: Option<TelemetrySpec>) {
        self.telemetry_config = _telemetry_config;
    }
}
