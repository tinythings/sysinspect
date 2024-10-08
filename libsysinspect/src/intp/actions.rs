use super::{
    actproc::{modfinder::ModCall, response::ActionResponse},
    inspector::SysInspector,
};
use crate::SysinspectError;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ModArgs {
    opts: Option<Vec<String>>,
    args: Option<HashMap<String, Vec<String>>>,
}

impl ModArgs {
    /// Return args
    pub fn args(&self) -> HashMap<String, Vec<String>> {
        if let Some(args) = &self.args {
            return args.to_owned();
        }
        HashMap::default()
    }

    /// Get options
    pub fn opts(&self) -> Vec<String> {
        let mut out = Vec::<String>::default();
        if let Some(optset) = &self.opts {
            for opt in optset {
                out.push(opt.to_owned());
            }
        }
        out
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Action {
    id: Option<String>, // NOTE: It is not optional, just added later!
    description: Option<String>,
    module: String,
    bind: Vec<String>,
    state: HashMap<String, ModArgs>,
    call: Option<ModCall>,
}

impl Action {
    pub fn new(id: &Value, states: &Value) -> Result<Self, SysinspectError> {
        let i_id: String;

        if let Some(id) = id.as_str() {
            i_id = id.to_string();
        } else {
            return Err(SysinspectError::ModelDSLError("No id found for an action".to_string()));
        }

        if let Ok(mut i) = serde_yaml::from_value::<Action>(states.to_owned()) {
            i.id = Some(i_id);
            Ok(i)
        } else {
            Err(SysinspectError::ModelDSLError(format!("Action {i_id} is misconfigured")))
        }
    }

    /// Get action's `id` field
    pub fn id(&self) -> String {
        self.id.to_owned().unwrap_or("".to_string())
    }

    /// Get action's `description` field
    pub fn descr(&self) -> String {
        self.description.to_owned().unwrap_or(format!("Action {}", self.id()))
    }

    /// Returns true if an action has a bind to an entity via its `eid` _(entity Id)_.
    pub fn binds_to(&self, eid: &str) -> bool {
        self.bind.contains(&eid.to_string())
    }

    /// Returns true if an action has requested state and is eligible to be processed.
    pub fn has_state(&self, sid: &str) -> bool {
        self.state.contains_key(sid)
    }

    /// Run action
    pub fn run(&self) -> Result<Option<ActionResponse>, SysinspectError> {
        if let Some(call) = &self.call {
            log::debug!("Calling action {} on state {}", self.id().yellow(), call.state().yellow());
            return call.run();
        }

        Ok(None)
    }

    /// Setup and activate an action and is done by the Inspector.
    /// This method finds module, sets up its parameters, binds constraint etc.
    pub(crate) fn setup(&mut self, inspector: &SysInspector, eid: &str, state: String) -> Result<Action, SysinspectError> {
        let mpath = inspector.cfg().get_module(&self.module)?;
        if let Some(mod_args) = self.state.get(&state) {
            let mut modcall = ModCall::default().set_state(state).set_module(mpath).set_aid(self.id()).set_eid(eid.to_string());

            // XXX: probably just pass args entirely at once instead, dropping add_kwargs() in a whole
            for (kw, arg) in &mod_args.args() {
                for a in arg {
                    modcall.add_kwargs(kw.to_owned(), a.to_owned());
                }
            }

            for opt in &mod_args.opts() {
                modcall.add_opt(opt.to_owned());
            }
            self.call = Some(modcall);
        }
        Ok(self.to_owned())
    }
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<Action> - Id: {}, Descr: {}, Module: {}, Active: {}",
            self.id(),
            self.descr(),
            self.module,
            self.call.is_some()
        )?;

        Ok(())
    }
}
