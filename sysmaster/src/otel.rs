use crate::registry::rec::MinionRecord;
use indexmap::IndexMap;
use libeventreg::kvdb::EventData;
use libsysinspect::mdescr::telemetry::{DataExportType, EventSelector, StaticDataDestination};
use libtelemetry::{
    otel_log_json,
    query::{cast_data, interpolate_data, load_data},
};
use serde_json::{Value, json, to_value};
use std::collections::HashMap;

/// OtelLogger is a struct that handles telemetry logging
/// using OpenTelemetry. It is responsible for processing telemetry data
/// and sending it to the OpenTelemetry collector.
///
/// It implements the `on_event` method to handle events and
/// log them using OpenTelemetry. It also provides methods to get telemetry selectors, response data,
/// attributes, and check if the telemetry data is compliant with the spec.
pub struct OtelLogger {
    buff: Vec<HashMap<String, serde_json::Value>>,
}

impl OtelLogger {
    pub fn new() -> Self {
        OtelLogger { buff: Vec::new() }
    }

    fn get_selectors(&self, pl: &HashMap<String, serde_json::Value>) -> Vec<EventSelector> {
        let tcf = match pl.get("telemetry").cloned() {
            Some(value) => match serde_json::from_value::<Vec<EventSelector>>(value) {
                Ok(selectors) => selectors,
                Err(err) => {
                    log::error!("Unable to parse telemetry config: {err}");
                    return Vec::new();
                }
            },
            None => {
                log::error!("Telemetry config not found");
                return Vec::new();
            }
        };

        tcf
    }

    fn get_response_data(&self, es: &EventSelector, pl: &HashMap<String, serde_json::Value>) -> IndexMap<String, Value> {
        let response = match pl.get("response").cloned() {
            Some(resp) => resp,
            None => {
                log::debug!("No response found");
                return IndexMap::new();
            }
        };

        match load_data(es.dataspec(), response.clone()) {
            Ok(data) => data,
            Err(err) => {
                log::debug!("Unable to load data: {err}");
                IndexMap::new()
            }
        }
    }

    fn get_attrs(&self, es: &EventSelector, data: &mut IndexMap<String, Value>) -> Vec<(String, Value)> {
        match es.export().static_destination() {
            StaticDataDestination::Attribute => es.export().static_data().iter().map(|(k, v)| (k.clone(), to_value(v).unwrap())).collect(),
            StaticDataDestination::Body => {
                data.extend(es.export().static_data().iter().map(|(k, v)| (k.to_string(), to_value(v).unwrap_or_default())));
                vec![]
            }
        }
    }

    /// Add data to the buffer for later processing with map/reduce.
    pub fn feed(&mut self, data: Vec<EventData>) {
        for e in data {
            let x = e.get_response();
            self.buff.push(x);
        }
    }

    // Emit log, depending on the type of event and the setup.
    pub fn log(&mut self, mrec: &MinionRecord, pl: &HashMap<String, serde_json::Value>, export_type: DataExportType) {
        if self.buff.is_empty() {
            self.on_event(mrec, pl, export_type);
        } else {
            self.on_model(mrec, pl, export_type);
        }
    }

    fn on_model(&mut self, mrec: &MinionRecord, pl: &HashMap<String, serde_json::Value>, _export_type: DataExportType) {
        log::info!("Event on model. Payload: {:#?}", pl);
        log::info!("Data: {:#?}", self.buff);
        self.buff.clear();
    }

    /// on_event is called when an a *minion action event* occurs (`RequestType::Event`).
    fn on_event(&self, mrec: &MinionRecord, pl: &HashMap<String, serde_json::Value>, export_type: DataExportType) {
        let tcf = self.get_selectors(pl);
        log::debug!("Telemetry config: {:#?}", tcf);

        for es in tcf {
            if es.is_model_event() {
                continue;
            }
            if es.export().event_type() != export_type {
                continue;
            }
            if !mrec.matches_selectors(es.select()) {
                log::debug!("Minion does not match traits selectors: {:#?}", es.dataspec());
                continue;
            }

            let mut rspdata = self.get_response_data(&es, pl);
            let attributes = self.get_attrs(&es, &mut rspdata);

            // Skip records if they do not match the telemetry data spec (i.e. no data for that particular selector)
            if !self.spec_compliant(&es, &rspdata) {
                log::debug!("Event does not match telemetry spec: {:#?}", es.dataspec());
                continue;
            }

            // Cast data
            cast_data(&mut rspdata, &es.export().cast_map());

            if es.export().telemetry_type().eq("log") {
                match es.export().attr_type().as_str() {
                    "string" => {
                        if let Some(tpl) = es.export().attr_format() {
                            match interpolate_data(&tpl, &rspdata) {
                                Ok(out) => otel_log_json(&json!(&out), attributes),
                                Err(err) => {
                                    log::error!("Unable to interpolate telemetry data: {err}");
                                    continue;
                                }
                            }
                        } else {
                            log::error!("Attribute type is set to \"string\", but no formatting template is provided.");
                            continue;
                        }
                    }
                    "json" => otel_log_json(&json!(rspdata), attributes),
                    _ => {
                        log::error!("Attribute type is set to \"{}\", but can be only \"string\" or \"json\".", es.export().telemetry_type());
                        continue;
                    }
                };
            } else {
                log::error!("Telemetry type {} is not supported or not yet implemented", es.export().telemetry_type());
            }
        }
    }

    fn spec_compliant(&self, es: &EventSelector, rspdata: &IndexMap<String, Value>) -> bool {
        for k in es.dataspec().keys() {
            if !rspdata.contains_key(k) {
                log::debug!("Missing key in telemetry data: {}", k);
                return false;
            }
        }
        true
    }
}
