use super::{actproc::modfinder::ModCall, inspector::SysInspector};
use crate::SysinspectError;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ModArgs {
    opts: Option<Vec<String>>,
    args: Option<Vec<HashMap<String, String>>>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Action {
    id: Option<String>,
    description: Option<String>,
    module: String,
    bind: Vec<String>,
    state: HashMap<String, ModArgs>,
    call: Option<ModCall>,
}

impl Action {
    pub fn new(id: &Value, states: &Value) -> Result<Self, SysinspectError> {
        let mut instance = Action::default();
        let i_id: String;

        if let Some(id) = id.as_str() {
            i_id = id.to_string();
        } else {
            return Err(SysinspectError::ModelDSLError("No id found for an action".to_string()));
        }

        if let Ok(mut i) = serde_yaml::from_value::<Action>(states.to_owned()) {
            i.id = Some(i_id);
            instance = i;
        }

        Ok(instance)
    }

    /// Get action's `id` field
    pub fn id(&self) -> String {
        self.id.to_owned().unwrap_or("".to_string())
    }

    /// Get action's `description` field
    pub fn descr(&self) -> String {
        self.description.to_owned().unwrap_or(format!("Action {}", self.id()))
    }

    /// Return assotiated module namespace.
    pub fn module_ns(&self) -> String {
        self.module.to_owned()
    }

    /// Returns true if an action has a bind to an entity via its `eid` _(entity Id)_.
    pub fn binds_to(&self, eid: &str) -> bool {
        self.bind.contains(&eid.to_string())
    }

    pub fn run(&self) {
        if let Some(call) = &self.call {
            log::debug!("Calling action {}", self.id().yellow());
        }
    }

    /// Setup and activate an action and is done by the Inspector.
    /// This method finds module, sets up its parameters, binds constraint etc.
    pub(crate) fn setup(&self, inspector: &SysInspector) -> Result<Action, SysinspectError> {
        let mpath = inspector.cfg().get_module(&self.module)?;
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
            self.module_ns(),
            self.call.is_some()
        )?;

        Ok(())
    }
}
