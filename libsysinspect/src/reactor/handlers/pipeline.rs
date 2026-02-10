use crate::{
    intp::{
        actproc::response::ActionResponse,
        conf::{EventConfig, EventConfigOption},
    },
    reactor::handlers::evthandler::EventHandler,
};
use colored::Colorize;
use indexmap::IndexMap;
use jsonpath_rust::JsonPath;
use libdpq::WorkItem;
use libsysproto::{MasterMessage, MinionTarget};
use serde::Deserialize;
use serde_json::json;
use serde_yaml::Value;
#[derive(Debug, Deserialize, Default)]
struct Call {
    query: String,
    #[serde(default)]
    context: IndexMap<String, Value>,
}

impl Call {
    fn context(&self) -> String {
        fn qstr(s: &str) -> String {
            let s = s.trim_end();
            let safe = !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'));

            if safe {
                s.to_string()
            } else {
                // single-quote and escape single quotes by doubling them
                format!("'{}'", s.replace('\'', "''"))
            }
        }

        self.context
            .iter()
            .map(|(k, v)| {
                let rendered = match v {
                    serde_yaml::Value::String(s) => qstr(s),
                    serde_yaml::Value::Number(n) => n.to_string(),
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    serde_yaml::Value::Null => "null".to_string(),
                    other => qstr(&PipelineHandler::scalar2s(other)),
                };
                format!("{k}:{rendered}")
            })
            .collect::<Vec<_>>()
            .join(",")
    }
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

    fn is_verbose(&self) -> bool {
        self.config().unwrap_or_default().get("verbose").and_then(|v| v.as_bool()).unwrap_or(false)
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

        for (k, v) in updates {
            let logv = Self::scalar2s(&v);
            call.context.insert(k.clone(), v);
            if self.is_verbose() {
                log::info!("[{}] Setting context variable {} to {}", PipelineHandler::id().bright_blue(), k.bright_green(), logv.bright_blue());
            }
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
        if self.is_verbose() {
            log::info!("[{}] handler received event {}", PipelineHandler::id().bright_blue(), evt.eid());
        }

        let Some(dpq) = crate::inspector::SysInspectRunner::dpq() else {
            log::error!("[{}]: DPQ not set", PipelineHandler::id().bright_blue());
            return;
        };

        // Skip events that don't belong
        if !evt.match_eid(&self.eid) {
            if self.is_verbose() {
                log::info!(
                    "[{}] Event {} doesn't match handler {}",
                    PipelineHandler::id().bright_blue(),
                    format!("{}/{}/{}/{}", evt.aid(), evt.eid(), evt.sid(), evt.response.retcode()).bright_yellow(),
                    self.eid.bright_yellow()
                );
            }
            return;
        }

        let calls = self.get_calls(evt);
        for call in calls {
            let mut target = MinionTarget::default();
            target.add_hostname("*");
            target.set_scheme(&call.query);
            target.set_context_query(&call.context());

            let mut msg = MasterMessage::command();
            msg.set_target(target);
            msg.payload();

            if let Err(e) = dpq.add(WorkItem::MasterCommand(msg)) {
                log::error!("[{}]: DPQ failed: {e}", PipelineHandler::id().bright_blue());
                return;
            }

            if self.is_verbose() {
                log::info!("[{}] added call to {}", PipelineHandler::id().bright_blue(), call.query.bright_yellow());
            }
        }
    }

    fn config(&self) -> Option<EventConfigOption> {
        self.config.cfg(&PipelineHandler::id())
    }
}
