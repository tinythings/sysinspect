use indexmap::IndexMap;
use libeventreg::kvdb;
use libsysinspect::{
    SysinspectError,
    cfg::mmconf::MinionConfig,
    mdescr::{mspec, mspecdef::ModelSpec, telemetry::TelemetrySpec},
};
use once_cell::sync::Lazy;
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

use crate::registry::rec::MinionRecord;

pub struct FunctionReducer {
    fmap: IndexMap<String, String>,
    data: IndexMap<String, Value>,
    model: Option<ModelSpec>,
    model_path: PathBuf,
    query: String,
    current_minion: Option<MinionRecord>,
}

impl FunctionReducer {
    pub fn new(model: PathBuf, query: String) -> Self {
        FunctionReducer { fmap: IndexMap::new(), data: IndexMap::new(), model: None, model_path: model, query, current_minion: None }
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
    pub(crate) fn reduce(&self) -> IndexMap<String, Value> {
        let out = IndexMap::new();
        if let Some(tspec) = self.get_tspec() {
            for evt_sel in tspec.cycle() {
                evt_sel.select();
            }

            log::info!("Model spec is here");
            log::info!("Data for reduction (length): {:#?}", self.data.len());
        }
        out
    }

    /// Sets current minion to accumulate data.
    pub(crate) fn set_current_minion(&mut self, mrec: &MinionRecord) {
        self.current_minion = Some(mrec.clone());
    }

    pub(crate) fn feed(&self, e: kvdb::EventData) {
        let mrec = match self.current_minion {
            Some(ref mrec) => mrec,
            None => return,
        };

        if let Some(tspec) = self.get_tspec() {
            for evt_sel in tspec.cycle() {
                if mrec.matches_selectors(evt_sel.select()) {
                    log::info!("Event matches dataspec selector: {:#?}", evt_sel.dataspec());
                    log::info!("Event data: {:#?}", e);
                    log::info!("-------------------------------");
                    // Now, for each event selector, we need to get the data
                    // and discard the rest.
                } else {
                    log::info!("Event does not match selector");
                }
            }
        }
    }

    /// Get the telemetry spec from the model
    fn get_tspec(&self) -> Option<TelemetrySpec> {
        self.model.as_ref().and_then(|mspec| mspec.telemetry())
    }
}
