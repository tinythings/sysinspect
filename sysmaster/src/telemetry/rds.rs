use crate::registry::rec::MinionRecord;
use indexmap::IndexMap;
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
    fmap: IndexMap<String, String>,
    mrecbuff: IndexMap<String, MinionRecord>,             // Minion traits
    rdata: IndexMap<String, Vec<HashMap<String, Value>>>, // response data, temporary buff
    mdata: IndexMap<String, HashMap<String, Vec<Value>>>, // Mapped data
    model: Option<ModelSpec>,
    model_path: PathBuf,
    query: String,
}

impl FunctionReducer {
    pub fn new(model: PathBuf, query: String) -> Self {
        FunctionReducer {
            fmap: IndexMap::new(),
            mrecbuff: IndexMap::new(),
            rdata: IndexMap::new(),
            model: None,
            model_path: model,
            query,
            mdata: IndexMap::new(),
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
    pub(crate) fn reduce(&self) {
        log::info!("Reducing data");
        log::info!("Data: {:?}", self.mdata);

        let tspec = match self.get_tspec() {
            Some(tspec) => tspec,
            None => {
                log::error!("Telemetry spec not found");
                return;
            }
        };

        let mut out: HashMap<String, Value> = HashMap::new();
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
                            if let Some(data) = data.get(&rkey) {
                                log::info!(">> {} => sum data: {:?}", rkey, data);
                                let mut sum = 0;
                                for v in data {
                                    if let Some(i) = v.as_i64() {
                                        sum += i;
                                    } else if let Some(f) = v.as_f64() {
                                        sum += f as i64;
                                    }
                                }
                                out.insert(mrec.id().to_string(), json!({"sum": sum}));
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
                            if let Some(data) = data.get(&rkey) {
                                log::info!(">> {} => average data: {:?}", rkey, data);
                                let mut sum = 0;
                                let mut count = 0;
                                for v in data {
                                    if let Some(i) = v.as_i64() {
                                        sum += i;
                                        count += 1;
                                    } else if let Some(f) = v.as_f64() {
                                        sum += f as i64;
                                        count += 1;
                                    }
                                }
                                if count > 0 {
                                    out.insert(mrec.id().to_string(), json!({"average": sum / count}));
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
                            if let Some(data) = data.get(&rkey) {
                                log::info!(">> {} => min data: {:?}", rkey, data);
                                let mut min = i64::MAX;
                                for v in data {
                                    if let Some(i) = v.as_i64() {
                                        min = min.min(i);
                                    } else if let Some(f) = v.as_f64() {
                                        min = min.min(f as i64);
                                    }
                                }
                                if min != i64::MAX {
                                    out.insert(mrec.id().to_string(), json!({"min": min}));
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
                            if let Some(data) = data.get(&rkey) {
                                log::info!(">> {} => max data: {:?}", rkey, data);
                                let mut max = i64::MIN;
                                for v in data {
                                    if let Some(i) = v.as_i64() {
                                        max = max.max(i);
                                    } else if let Some(f) = v.as_f64() {
                                        max = max.max(f as i64);
                                    }
                                }
                                if max != i64::MIN {
                                    out.insert(mrec.id().to_string(), json!({"max": max}));
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
        for (mid, v_rdata) in &self.rdata {
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
                    if mrec.matches_selectors(selector.select()) {
                        for (dskey, jpath) in selector.dataspec() {
                            if let Ok(matches) = select(&jpath, &json!(rdata)) {
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
        self.rdata.clear();
    }

    /// Add data to the reducer.
    pub(crate) fn feed(&mut self, mrec: MinionRecord, rdata: HashMap<String, Value>) {
        self.mrecbuff.entry(mrec.id().to_string()).or_insert_with(|| mrec.clone());
        self.rdata.entry(mrec.id().to_string()).and_modify(|vec| vec.push(rdata.clone())).or_insert(vec![rdata.clone()]);
    }
}
