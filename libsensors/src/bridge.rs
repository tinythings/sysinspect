use libsysinspect::intp::actproc::response::ActionResponse;
use libsysinspect::reactor::evtproc::EventProcessor;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Build an emitter closure that converts a sensor JSON event into an ActionResponse
/// and registers it into the Sysinspect reactor.
///
/// This is the Sysinspect boundary. Everything "sensor runtime" should call this,
/// not re-implement it.
pub fn reactor_emitter(sid: String, reactor: Option<Arc<Mutex<EventProcessor>>>) -> impl Fn(JsonValue) + Send + Sync + Clone + 'static {
    move |ev: JsonValue| {
        log::debug!("Registering event from sensor to reactor: {ev}");

        let Some(reactor) = reactor.clone() else {
            log::warn!("No reactor attached for sensor '{}': {}", sid, ev);
            return;
        };

        let eid = ev.get("eid").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| sid.clone());

        tokio::spawn(async move {
            let response = ActionResponse::from_sensor(ev);
            reactor.lock().await.receiver().register(eid, response);
        });
    }
}
