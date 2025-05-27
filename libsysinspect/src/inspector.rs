use crate::{
    SysinspectError,
    cfg::mmconf::MinionConfig,
    intp::{self, actions::Action, inspector::SysInspector},
    mdescr::mspec,
    reactor::{callback::EventProcessorCallback, evtproc::EventProcessor},
    traits::systraits::SystemTraits,
};
use colored::Colorize;
use intp::actproc::response::ActionResponse;
use once_cell::sync::OnceCell;
use std::sync::Arc;

static MINION_CONFIG: OnceCell<Arc<MinionConfig>> = OnceCell::new();

#[derive(Debug, Default)]
pub struct SysInspectRunner {
    model_pth: String,
    state: Option<String>,
    entities: Vec<String>,

    // Check book labels
    cb_labels: Vec<String>,

    // Minion traits, if running in distributed mode
    traits: Option<SystemTraits>,

    // Constraints evaluation results ID/outcome.
    cstr_f: Vec<String>, // constraints that failed
    cstr_s: Vec<String>, // constraints that succeeded

    // Called after every action
    action_callbacks: Vec<Box<dyn EventProcessorCallback>>,

    // Called after all actions at the end
    model_callbacks: Vec<Box<dyn EventProcessorCallback>>,
}

impl SysInspectRunner {
    pub fn new(cfg: &MinionConfig) -> SysInspectRunner {
        MINION_CONFIG.get_or_init(|| Arc::new(cfg.to_owned()));
        SysInspectRunner { ..Default::default() }
    }

    /// Get Minion Config
    pub fn minion_cfg() -> Arc<MinionConfig> {
        MINION_CONFIG.get().unwrap_or(&Arc::new(MinionConfig::default())).clone()
    }

    /// Adds a callback to be called after every action
    pub fn add_action_callback(&mut self, c: Box<dyn EventProcessorCallback>) {
        self.action_callbacks.push(c);
    }

    /// Adds a callback to be called after all actions at the end of the model cycle
    pub fn add_model_callback(&mut self, c: Box<dyn EventProcessorCallback>) {
        self.model_callbacks.push(c);
    }

    /// Return minion config as JSON
    pub fn minion_cfg_json() -> serde_json::Value {
        serde_json::to_value(&*Self::minion_cfg()).unwrap_or_default()
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

    /// Verify if an action can proceed
    fn action_allowed(&self, a: &Action) -> Result<bool, SysinspectError> {
        log::info!("Running {}", a.id().yellow());

        for c in a.if_false() {
            if !self.cstr_s.contains(&c) && !self.cstr_f.contains(&c) {
                return Err(SysinspectError::ModelDSLError(format!(
                    "Constraint {} expected to be already failed. Please fix your model.",
                    c
                )));
            }

            if !self.cstr_f.contains(&c) {
                return Ok(false);
            }
        }

        for c in a.if_true() {
            if !self.cstr_s.contains(&c) && !self.cstr_f.contains(&c) {
                return Err(SysinspectError::ModelDSLError(format!(
                    "Constraint {} expected to be already succeeded. Please fix your model.",
                    c
                )));
            }

            if !self.cstr_s.contains(&c) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Update action response
    fn update_cstr_eval(&mut self, r: &ActionResponse) {
        // Record the action
        for r in r.constraints.failures() {
            self.cstr_f.push(r.id.to_owned());
        }
        for r in r.constraints.passes() {
            self.cstr_s.push(r.id.to_owned());
        }
    }

    /// Start the inspector
    pub async fn start(&mut self) {
        log::info!("Starting sysinspect runner");
        match mspec::load(Self::minion_cfg().clone(), &self.model_pth, self.traits.clone()) {
            Ok(spec) => {
                log::info!("Model spec loaded");
                match SysInspector::new(spec.clone(), Some(Self::minion_cfg().sharelib_dir().clone())) {
                    Ok(isp) => {
                        // Setup event processor
                        let mut evtproc = EventProcessor::new().set_config(isp.cfg(), spec.telemetry());
                        for c in std::mem::take(&mut self.action_callbacks) {
                            evtproc.add_action_callback(c);
                        }
                        for c in std::mem::take(&mut self.model_callbacks) {
                            evtproc.add_model_callback(c);
                        }

                        let actions = if !self.cb_labels.is_empty() {
                            isp.actions_by_relations(self.cb_labels.to_owned(), self.state.to_owned())
                        } else {
                            isp.actions_by_entities(self.entities.to_owned(), self.state.to_owned())
                        };

                        match actions {
                            Ok(actions) => {
                                for ac in actions {
                                    match self.action_allowed(&ac) {
                                        Ok(is_allowed) => {
                                            if is_allowed {
                                                match ac.run() {
                                                    Ok(response) => {
                                                        let response = response.unwrap_or(ActionResponse::default());
                                                        self.update_cstr_eval(&response);
                                                        evtproc.receiver().register(response.eid().to_owned(), response);
                                                    }
                                                    Err(err) => {
                                                        log::error!("{err}")
                                                    }
                                                }
                                            } else {
                                                log::warn!("Action {} skipped due to dependencies results mismatch", ac.id())
                                            }
                                        }
                                        Err(err) => {
                                            log::error!("{err}");
                                            return; // halt immediately
                                        }
                                    };
                                }
                            }
                            Err(err) => {
                                log::error!("{}", err);
                            }
                        }
                        log::info!("Starting event processor cycle");
                        evtproc.process().await;
                        log::info!("Event processing cycle finished");
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
