use std::path::PathBuf;

use crate::{
    cfg::mmconf::DEFAULT_SHARELIB,
    intp::{self, inspector::SysInspector},
    mdescr::mspec,
    reactor::evtproc::EventProcessor,
    traits::systraits::SystemTraits,
};
use intp::actproc::response::ActionResponse;

#[derive(Debug, Default)]
pub struct SysInspectRunner {
    model_pth: String,
    state: Option<String>,
    entities: Vec<String>,

    // Check book labels
    cb_labels: Vec<String>,

    // Minion traits, if running in distributed mode
    traits: Option<SystemTraits>,

    // Sharelib
    sharelib: PathBuf,
}

impl SysInspectRunner {
    pub fn new(sharelib: Option<PathBuf>) -> SysInspectRunner {
        SysInspectRunner { sharelib: sharelib.unwrap_or(PathBuf::from(DEFAULT_SHARELIB)), ..Default::default() }
    }

    /// Set model path
    pub fn set_model_path(&mut self, p: &str) {
        self.model_pth = p.to_string()
    }

    /// Set process state
    pub fn set_state(&mut self, state: Option<String>) {
        self.state = state;
    }

    /// Set entities to query
    pub fn set_entities(&mut self, entities: Vec<String>) {
        self.entities = entities;
    }

    /// Set checkbook labels
    pub fn set_checkbook_labels(&mut self, labels: Vec<String>) {
        self.cb_labels = labels;
    }

    pub fn start(&self) {
        log::info!("Starting sysinspect runner");
        match mspec::load(&self.model_pth, self.traits.clone()) {
            Ok(spec) => {
                log::debug!("Initalising inspector");
                match SysInspector::new(spec, Some(self.sharelib.clone())) {
                    Ok(isp) => {
                        // Setup event processor
                        let mut evtproc = EventProcessor::new().set_config(isp.cfg());

                        let actions = if !self.cb_labels.is_empty() {
                            isp.actions_by_relations(self.cb_labels.to_owned(), self.state.to_owned())
                        } else {
                            isp.actions_by_entities(self.entities.to_owned(), self.state.to_owned())
                        };

                        match actions {
                            Ok(actions) => {
                                for ac in actions {
                                    match ac.run() {
                                        Ok(response) => {
                                            let response = response.unwrap_or(ActionResponse::default());
                                            evtproc.receiver().register(response.eid().to_owned(), response);
                                        }
                                        Err(err) => {
                                            log::error!("{err}")
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("{}", err);
                            }
                        }
                        evtproc.process();
                    }
                    Err(err) => log::error!("{err}"),
                }
                log::debug!("Done");
            }
            Err(err) => log::error!("Error loading mspec: {}", err),
        };
    }

    pub fn set_traits(&mut self, traits: SystemTraits) {
        self.traits = Some(traits);
    }
}
