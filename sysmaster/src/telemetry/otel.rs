use crate::{registry::rec::MinionRecord, telemetry::map::FunctionMapper};
use indexmap::IndexMap;
use libeventreg::kvdb::EventData;
use libsysinspect::{
    SysinspectError,
    mdescr::telemetry::{DataExportType, EventSelector, StaticDataDestination},
};
use libtelemetry::{
    otel_log_json,
    query::{cast_data, interpolate_data, load_data},
};
use serde_json::{Value, json, to_value};
use std::collections::HashMap;

/// Eventmap is a placeholder struct for telemetry event mapping.
/// This is merely done to avoid monstrosity constructions like
/// map of maps of maps...
#[derive(Debug, Clone)]
struct Eventmap {
    mrec: MinionRecord,
    events: Vec<EventData>,
}

impl Eventmap {
    pub fn new(mrec: MinionRecord, events: Vec<EventData>) -> Self {
        Eventmap { mrec, events }
    }

    /// Get minion record
    pub fn minion_record(&self) -> &MinionRecord {
        &self.mrec
    }

    /// Get events
    pub fn events(&self) -> &Vec<EventData> {
        &self.events
    }
}

/// OtelLogger is a struct that handles telemetry logging
/// using OpenTelemetry. It is responsible for creating, aggregating
/// and further processing telemetry data, sending it to the OpenTelemetry collector.
///
/// It implements the `on_event` and `on_model` methods to handle events and
/// log them using OpenTelemetry. It also provides methods to get telemetry selectors, response data,
/// attributes, and check if the telemetry data is compliant with the model spec.
pub struct OtelLogger {
    buff: Vec<Eventmap>,
    selectors: Vec<EventSelector>,
    payload: HashMap<String, serde_json::Value>,
    map: bool,
}

impl OtelLogger {
    pub fn new(payload: &HashMap<String, serde_json::Value>) -> Self {
        OtelLogger { buff: Vec::new(), selectors: Self::get_selectors(payload), payload: payload.clone(), map: false }
    }

    fn get_selectors(pl: &HashMap<String, serde_json::Value>) -> Vec<EventSelector> {
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
                log::error!("No response found");
                return IndexMap::new();
            }
        };

        match load_data(es.dataspec(), response.clone()) {
            Ok(data) => data,
            Err(err) => {
                match err {
                    SysinspectError::JsonPathInfo(err) => {
                        log::debug!("Data does not match selector: {:#?}, error: {}", es.dataspec(), err);
                    }
                    _ => {
                        log::error!("Unable to load selector data: {}", err);
                    }
                }
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

    /// Set map flag
    pub fn set_map(&mut self, map: bool) {
        self.map = map;
    }

    /// Add data to the buffer for later processing with map/reduce.
    pub fn feed(&mut self, data: Vec<EventData>, mrec: MinionRecord) {
        self.buff.push(Eventmap::new(mrec, data.into_iter().collect()));
    }

    // Emit log, depending on the type of event and the setup.
    pub fn log(&mut self, mrec: &MinionRecord, export_type: DataExportType) {
        if self.buff.is_empty() {
            self.on_event(mrec, export_type);
        } else if self.map {
            self.on_map();
        }
    }

    /// on_map is called when a *minion model* event occurs (`RequestType::Model`).
    fn on_map(&mut self) {
        // Map
        for s in &self.selectors {
            for em in &self.buff {
                if em.minion_record().matches_selectors(s.select()) {
                    // Get the data using given selector. Drop messages if data is not matching it.
                    for pl in em.events() {
                        let actions = s.filter().actions();
                        if !actions.is_empty() && !actions.iter().any(|a| a.eq(&pl.get_action_id())) {
                            log::debug!("Action {} not in {}, skipping", pl.get_action_id(), actions.join(", "));
                            continue;
                        }
                        if !pl.get_entity_id().eq(&s.filter().entity()) {
                            log::debug!("Entity ID {} does not match {}, skipping", pl.get_entity_id(), s.filter().entity());
                            continue;
                        }

                        match load_data(s.dataspec(), serde_json::Value::Object(pl.clone().get_response().into_iter().collect())) {
                            Ok(data) => {
                                let mut mdata = FunctionMapper::new(s.map()).set_data(data).map();
                                let attributes = self.get_attrs(s, &mut mdata);

                                if self.spec_compliant(s, &mdata) {
                                    self.emit(s, &mut mdata, attributes);
                                } else {
                                    log::warn!("Data does not match telemetry spec: {:#?}", s.dataspec());
                                }
                            }
                            Err(err) => match err {
                                SysinspectError::JsonPathInfo(err) => {
                                    log::debug!("Data does not match selector: {:#?}, error: {}", s.dataspec(), err);
                                }
                                _ => {
                                    log::error!("Unable to load selector data: {}", err);
                                }
                            },
                        }
                    }
                }
            }
        }

        self.buff.clear();
    }

    /// on_event is called when an a *minion action event* occurs (`RequestType::Event`).
    fn on_event(&self, mrec: &MinionRecord, export_type: DataExportType) {
        for es in &self.selectors {
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

            let entity_id = self.payload.get("eid").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            if !entity_id.is_empty() && !es.filter().entity().is_empty() && !es.filter().entity().eq(&entity_id) {
                log::debug!("Event ID {} does not match entity filter: {}", entity_id, es.filter().entity());
                continue;
            }

            let action_id = self.payload.get("aid").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            if !action_id.is_empty() && !es.filter().actions().is_empty() && !es.filter().actions().iter().any(|a| a.eq(&action_id)) {
                log::debug!("Action ID {} does not match filter: {}", action_id, es.filter().actions().join(", "));
                continue;
            }

            let mut rspdata = self.get_response_data(es, &self.payload);
            let attributes = self.get_attrs(es, &mut rspdata);

            // Skip records if they do not match the telemetry data spec (i.e. no data for that particular selector)
            if !self.spec_compliant(es, &rspdata) {
                log::debug!("Event does not match telemetry spec: {:#?}", es.dataspec());
                continue;
            }

            self.emit(es, &mut rspdata, attributes);
        }
    }

    /// Emit telemetry data to OpenTelemetry collector.
    fn emit(&self, es: &EventSelector, rspdata: &mut IndexMap<String, Value>, attributes: Vec<(String, Value)>) {
        cast_data(rspdata, &es.export().cast_map());

        if es.export().telemetry_type().eq("log") {
            match es.export().attr_type().as_str() {
                "string" => {
                    if let Some(tpl) = es.export().attr_format() {
                        match interpolate_data(&tpl, rspdata) {
                            Ok(out) => otel_log_json(&json!(&out), attributes),
                            Err(err) => {
                                log::error!("Unable to interpolate telemetry data: {err}");
                            }
                        }
                    } else {
                        log::error!("Attribute type is set to \"string\", but no formatting template is provided.");
                    }
                }
                "json" => otel_log_json(&json!(rspdata), attributes),
                _ => {
                    log::error!("Attribute type is set to \"{}\", but can be only \"string\" or \"json\".", es.export().telemetry_type());
                }
            }
        } else {
            log::error!("Telemetry type {} is not supported or not yet implemented", es.export().telemetry_type());
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
