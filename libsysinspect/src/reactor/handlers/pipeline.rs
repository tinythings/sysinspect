use colored::Colorize;
use indexmap::IndexMap;
use jsonpath_rust::JsonPath;
use libdpq::WorkItem;
use libsysproto::{MasterMessage, MinionTarget};
use serde::Deserialize;
use serde_json::json;
use serde_yaml::Value;

use crate::{
    intp::{
        actproc::response::ActionResponse,
        conf::{EventConfig, EventConfigOption},
    },
    reactor::handlers::evthandler::EventHandler,
};
#[derive(Debug, Deserialize, Default)]
struct Call {
    query: String,
    #[serde(default)]
    context: IndexMap<String, Value>,
}

#[derive(Default, Debug)]
pub struct PipelineHandler {
    eid: String,
    config: EventConfig,
}

impl PipelineHandler {
    fn get_calls(&self, evt: &ActionResponse) -> Vec<Call> {
        let cfg = self.config().unwrap_or_default();
        match cfg.get("calls") {
            Some(Value::Sequence(_)) => {
                let mut calls: Vec<Call> = serde_yaml::from_value(cfg.get("calls").clone().unwrap_or(Value::Null)).unwrap_or_default();

                for call in &mut calls {
                    self.eval_context(evt, call);
                }
                calls
            }
            _ => vec![],
        }
    }

    fn scalar2s(v: &Value) -> String {
        match v {
            Value::String(s) => s.trim_end().to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "".to_string(),
            _ => "".to_string(),
        }
    }

    fn eval_context(&self, evt: &ActionResponse, call: &mut Call) {
        let data = evt.response.data().unwrap_or(json!({}));

        let updates: Vec<(String, Value)> = call
            .context
            .iter()
            .map(|(k, v_yaml)| {
                // 1) YAML scalar → clean string JSONPath
                let path = Self::scalar2s(v_yaml);

                // 2) Run JSONPath
                let out = match data.query(&path) {
                    Ok(h) if !h.is_empty() => match &h[0] {
                        serde_json::Value::String(s) => Value::String(s.clone()),
                        serde_json::Value::Number(n) => Value::Number(serde_yaml::Number::from(n.as_f64().unwrap_or(0.0))),
                        serde_json::Value::Bool(b) => Value::Bool(*b),
                        _ => Value::Null,
                    },
                    _ => Value::Null,
                };

                (k.clone(), out)
            })
            .collect();

        // 3) Apply updates
        for (k, v) in updates {
            let logv = Self::scalar2s(&v);
            call.context.insert(k.clone(), v);
            log::info!("Setting context variable {} to {}", k.bright_green(), logv.bright_blue());
        }
    }
}

impl EventHandler for PipelineHandler {
    fn new(eid: String, cfg: EventConfig) -> Self
    where
        Self: Sized,
    {
        PipelineHandler { eid, config: cfg }
    }

    fn id() -> String
    where
        Self: Sized,
    {
        "pipeline".to_string()
    }

    fn handle(&self, evt: &ActionResponse) {
        log::info!("Pipeline handler received event {}", evt.eid());
        let Some(dpq) = crate::inspector::SysInspectRunner::dpq() else {
            log::error!("pipeline: DPQ not set");
            return;
        };

        // Skip events that don't belong
        if !evt.match_eid(&self.eid) {
            log::info!("Event {} doesn't match handler {}", evt.eid().bright_yellow(), self.eid.bright_yellow());
            return;
        }

        let calls = self.get_calls(evt);
        log::info!("Calls: {:#?}", calls);

        log::info!("Action response data: {:#?}", evt.response.data());

        let mut target = MinionTarget::default();
        //target.add_hostname(&traits::get_minion_traits(None).get("system.hostname.fqdn").map(|v| v.to_string()).unwrap_or("*".to_string()));
        target.add_hostname("*");
        target.set_scheme("single/wasm");
        target.set_context_query("");

        let mut msg = MasterMessage::command();
        msg.set_target(target);
        msg.payload();

        // Stupid bypass for now
        if let Err(e) = dpq.add(WorkItem::MasterCommand(msg)) {
            log::error!("pipeline: dpq enqueue failed: {e}");
            return;
        }
        log::info!("pipeline: enqueued ping to DPQ with eid {}", self.eid);
    }

    fn config(&self) -> Option<EventConfigOption> {
        self.config.cfg(&PipelineHandler::id())
    }
}
