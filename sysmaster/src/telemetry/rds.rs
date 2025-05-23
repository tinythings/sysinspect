use crate::registry::rec::MinionRecord;
use indexmap::IndexMap;
use libeventreg::kvdb::EventData;
use libsysinspect::{
    SysinspectError,
    cfg::mmconf::MinionConfig,
    mdescr::{mspec, mspecdef::ModelSpec, telemetry::TelemetrySpec},
};
use libtelemetry::query::select;
use once_cell::sync::Lazy;
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub struct FunctionReducer {
    mrecbuff: IndexMap<String, MinionRecord>,             // Minion traits
    raw_data: IndexMap<String, Vec<EventData>>,           // response data, temporary buff
    rdata: IndexMap<String, Value>,                       // reduced data
    mdata: IndexMap<String, HashMap<String, Vec<Value>>>, // Mapped data
    model: Option<ModelSpec>,
    model_path: PathBuf,
    query: String,
}

impl FunctionReducer {
    pub fn new(model: PathBuf, query: String) -> Self {
        FunctionReducer {
            mrecbuff: IndexMap::new(),
            raw_data: IndexMap::new(),
            rdata: IndexMap::new(),
            model: None,
            model_path: model,
            mdata: IndexMap::new(),
            query,
        }
    }

    /// Get the telemetry spec from the model
    fn get_tspec(&self) -> Option<TelemetrySpec> {
        self.model.as_ref().and_then(|mspec| mspec.telemetry())
    }

    /// Load the model from the path
    /// and cache it in the provided cache.
    /// If the model is already in the cache, it will be used instead of loading it again.
    pub async fn load_model(mut self, cache: &Lazy<Arc<Mutex<HashMap<PathBuf, ModelSpec>>>>) -> Result<Self, SysinspectError> {
        let mut c = cache.lock().await;
        match c.get(&self.model_path) {
            Some(m) => {
                self.model = Some(m.clone());
            }
            None => {
                self.model = Some(mspec::load(MinionConfig::default().into(), self.model_path.to_str().unwrap(), None)?);
                c.insert(self.model_path.clone(), self.model.clone().unwrap());
            }
        };

        Ok(self)
    }

    /// Run the function over values.
    /// Consume self, apply each mapping in-place to `self.data`, and return it.
    pub(crate) fn reduce(&mut self) {
        let tspec = match self.get_tspec() {
            Some(tspec) => tspec,
            None => {
                log::error!("Telemetry spec not found");
                return;
            }
        };

        for selector in tspec.cycle() {
            for (rkey, rfunc) in selector.reduce() {
                match rfunc.as_str() {
                    "sum" => {
                        for (mid, data) in &self.mdata {
                            let mrec = match self.mrecbuff.get(mid) {
                                Some(mrec) => mrec,
                                None => {
                                    log::error!("Minion ID {} was not found in correlation to the map/reduce. This should not have happened.", mid);
                                    continue;
                                }
                            };
                            if let Some(mapdata) = data.get(&rkey) {
                                let mut nsum: i64 = 0;
                                let mut concat: Vec<String> = Vec::new();
                                for v in mapdata {
                                    if let Some(i) = v.as_i64() {
                                        nsum += i;
                                    } else if let Some(f) = v.as_f64() {
                                        nsum += f as i64;
                                    } else if let Some(b) = v.as_bool() {
                                        nsum += if b { 1 } else { 0 };
                                    } else if let Some(s) = v.as_str() {
                                        concat.push(s.to_string());
                                    }
                                }
                                let mut result = serde_json::Map::new();
                                result.insert("query".to_string(), json!(self.query));
                                if concat.is_empty() {
                                    result.insert("sum".to_string(), json!(nsum));
                                } else {
                                    result.insert("sum".to_string(), json!(concat.join(" ")));
                                }
                                // Add static data
                                for (key, value) in selector.export().static_data() {
                                    result.insert(key, serde_json::to_value(value).unwrap());
                                }

                                // Add map results
                                for (k, v) in data {
                                    if k != &rkey {
                                        result.insert(k.clone(), json!(v));
                                    }
                                }

                                self.rdata.insert(mrec.id().to_string(), json!(result));
                            }
                        }
                    }

                    "average" | "avg" => {
                        for (mid, data) in &self.mdata {
                            let mrec = match self.mrecbuff.get(mid) {
                                Some(mrec) => mrec,
                                None => {
                                    log::error!("Minion ID {} was not found in correlation to the map/reduce. This should not have happened.", mid);
                                    continue;
                                }
                            };
                            if let Some(mapdata) = data.get(&rkey) {
                                let mut sum = 0.0;
                                let mut count = 0;
                                for v in mapdata {
                                    if let Some(i) = v.as_i64() {
                                        sum += i as f64;
                                        count += 1;
                                    } else if let Some(f) = v.as_f64() {
                                        sum += f;
                                        count += 1;
                                    }
                                }
                                if count > 0 {
                                    let mut result = serde_json::Map::new();
                                    result.insert("average".to_string(), json!(sum / count as f64));
                                    result.insert("query".to_string(), json!(self.query));
                                    for (key, value) in selector.export().static_data() {
                                        result.insert(key, serde_json::to_value(value).unwrap());
                                    }

                                    // Add map results
                                    for (k, v) in data {
                                        if k != &rkey {
                                            result.insert(k.clone(), json!(v));
                                        }
                                    }

                                    self.rdata.insert(mrec.id().to_string(), serde_json::Value::Object(result));
                                }
                            }
                        }
                    }

                    "min" => {
                        for (mid, data) in &self.mdata {
                            let mrec = match self.mrecbuff.get(mid) {
                                Some(mrec) => mrec,
                                None => {
                                    log::error!("Minion ID {} was not found in correlation to the map/reduce. This should not have happened.", mid);
                                    continue;
                                }
                            };
                            if let Some(mapdata) = data.get(&rkey) {
                                let mut min = i64::MAX;
                                for v in mapdata {
                                    if let Some(i) = v.as_i64() {
                                        min = min.min(i);
                                    } else if let Some(f) = v.as_f64() {
                                        min = min.min(f as i64);
                                    }
                                }
                                if min != i64::MAX {
                                    let mut result = serde_json::Map::new();
                                    result.insert("min".to_string(), json!(min));
                                    result.insert("query".to_string(), json!(self.query));
                                    for (key, value) in selector.export().static_data() {
                                        result.insert(key, serde_json::to_value(value).unwrap());
                                    }
                                    // Add map results
                                    for (k, v) in data {
                                        if k != &rkey {
                                            result.insert(k.clone(), json!(v));
                                        }
                                    }
                                    self.rdata.insert(mrec.id().to_string(), json!(result));
                                }
                            }
                        }
                    }

                    "max" => {
                        for (mid, data) in &self.mdata {
                            let mrec = match self.mrecbuff.get(mid) {
                                Some(mrec) => mrec,
                                None => {
                                    log::error!("Minion ID {} was not found in correlation to the map/reduce. This should not have happened.", mid);
                                    continue;
                                }
                            };
                            if let Some(mapdata) = data.get(&rkey) {
                                let mut max = i64::MIN;
                                for v in mapdata {
                                    if let Some(i) = v.as_i64() {
                                        max = max.max(i);
                                    } else if let Some(f) = v.as_f64() {
                                        max = max.max(f as i64);
                                    }
                                }
                                if max != i64::MIN {
                                    let mut result = serde_json::Map::new();
                                    result.insert("max".to_string(), json!(max));
                                    result.insert("query".to_string(), json!(self.query));
                                    for (key, value) in selector.export().static_data() {
                                        result.insert(key, serde_json::to_value(value).unwrap());
                                    }
                                    // Add map results
                                    for (k, v) in data {
                                        if k != &rkey {
                                            result.insert(k.clone(), json!(v));
                                        }
                                    }
                                    self.rdata.insert(mrec.id().to_string(), json!(result));
                                }
                            }
                        }
                    }
                    _ => {} // noop
                }
            }
        }
    }

    /// Map the data to a new value.
    pub(crate) fn map(&mut self) {
        let tspec = self.get_tspec();
        if tspec.is_none() {
            log::warn!("No telemetry spec found in model");
            return;
        }
        let tspec = tspec.unwrap();

        // Select data for mapping
        for (mid, v_rdata) in &self.raw_data {
            let mrec = match self.mrecbuff.get(mid) {
                Some(mrec) => mrec,
                None => {
                    log::error!("Minion ID {} was not found in correlation to the map/reduce. This should not have happened.", mid);
                    continue;
                }
            };
            let mut mdata: HashMap<String, Vec<Value>> = HashMap::new();
            for rdata in v_rdata {
                for selector in tspec.cycle() {
                    // If event Id specified
                    let entity_id = selector.filter().entity();
                    if !entity_id.is_empty() && !entity_id.eq(&rdata.get_entity_id()) {
                        continue;
                    }

                    // If filter actions are specified
                    let actions = selector.filter().actions();
                    if !actions.is_empty() && !actions.iter().any(|a| a.eq(&rdata.get_action_id())) {
                        continue;
                    }

                    if !mdata.get("entity").map_or(false, |v| v.contains(&json!(rdata.get_entity_id()))) {
                        mdata.entry("entity".to_string()).or_default().push(json!(rdata.get_entity_id()));
                    }
                    if !mdata.get("action").map_or(false, |v| v.contains(&json!(rdata.get_action_id()))) {
                        mdata.entry("action".to_string()).or_default().push(json!(rdata.get_action_id()));
                    }

                    if mrec.matches_selectors(selector.select()) {
                        for (dskey, jpath) in selector.dataspec() {
                            if let Ok(matches) = select(&jpath, &json!(rdata.get_response())) {
                                if !matches.is_empty() {
                                    mdata.entry(dskey.clone()).or_default().push(matches[0].clone());
                                }
                            }
                        }
                    }
                }
            }
            self.mdata.insert(mid.clone(), mdata);
        }

        // Purge original data and free memory
        self.raw_data.clear();

        // Map the data
        for (_, data) in &mut self.mdata {
            for selector in tspec.cycle() {
                for (mkey, mfunc) in selector.map() {
                    match mfunc.as_str() {
                        "round" => {
                            if let Some(values) = data.get_mut(&mkey) {
                                for value in values {
                                    *value = json!(value.as_f64().map(|v| v.round()).unwrap_or(0.0));
                                }
                            }
                        }
                        "as-int" => {
                            if let Some(values) = data.get_mut(&mkey) {
                                for value in values {
                                    *value = json!(match value {
                                        Value::String(s) => s.parse::<i64>().unwrap_or(0),
                                        Value::Number(n) => n.as_i64().unwrap_or(0),
                                        _ => value.as_i64().unwrap_or(0),
                                    });
                                }
                            }
                        }

                        "as-float" => {
                            if let Some(values) = data.get_mut(&mkey) {
                                for value in values {
                                    *value = json!(match value {
                                        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
                                        Value::Number(n) => n.as_f64().unwrap_or(0.0),
                                        _ => value.as_f64().unwrap_or(0.0),
                                    });
                                }
                            }
                        }

                        "as-bool" => {
                            if let Some(values) = data.get_mut(&mkey) {
                                for value in values {
                                    *value = json!(match value {
                                        Value::String(s) => s.parse::<bool>().unwrap_or(false),
                                        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
                                        Value::Bool(b) => *b,
                                        _ => false,
                                    });
                                }
                            }
                        }

                        "as-str" => {
                            if let Some(values) = data.get_mut(&mkey) {
                                for value in values {
                                    *value = json!(match value {
                                        Value::String(s) => s.clone(),
                                        Value::Number(n) => n.to_string(),
                                        _ => value.to_string(),
                                    });
                                }
                            }
                        }
                        _ => {} // noop
                    }
                }
            }
        }
    }

    /// Add data to the reducer.
    pub(crate) fn feed(&mut self, mrec: MinionRecord, event: EventData) {
        self.mrecbuff.entry(mrec.id().to_string()).or_insert_with(|| mrec.clone());
        self.raw_data.entry(mrec.id().to_string()).and_modify(|vec| vec.push(event.clone())).or_insert(vec![event.clone()]);
    }

    /// Get the reduced data
    pub fn get_reduced_data(&self) -> &IndexMap<String, Value> {
        &self.rdata
    }

    /// Get the mapped data
    pub fn get_mapped_data(&self) -> &IndexMap<String, HashMap<String, Vec<Value>>> {
        &self.mdata
    }
}
