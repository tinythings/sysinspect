use crate::{modlib::response::ModResponse, SysinspectError};
use core::str;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    fmt::Display,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use super::response::ActionResponse;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModCall {
    state: String,
    module: PathBuf,
    args: HashMap<String, String>,
    opts: Vec<String>,
}

impl ModCall {
    /// Set state
    pub fn set_state(mut self, state: String) -> Self {
        self.state = state;
        self
    }

    /// Set resolved module physical path
    pub fn set_module(mut self, modpath: PathBuf) -> Self {
        self.module = modpath;
        self
    }

    /// Add a pair of kwargs
    pub fn add_kwargs(&mut self, kw: String, arg: String) -> &mut Self {
        self.args.insert(kw, arg);
        self
    }

    /// Add an option
    pub fn add_opt(&mut self, opt: String) -> &mut Self {
        self.opts.push(opt);
        self
    }

    /// Serialise args and opts to a JSON string for the call.
    fn params_json(&self) -> String {
        let mut out: HashMap<String, serde_json::Value> = HashMap::default();
        if !self.args.is_empty() {
            out.insert("arguments".to_string(), json!(self.args));
        }

        if !self.opts.is_empty() {
            out.insert("options".to_string(), json!(self.opts));
        }

        let x = json!(out).to_string();
        log::trace!("Params: {}", x);
        x
    }

    pub fn run(&self) -> Result<Option<ActionResponse>, SysinspectError> {
        // TODO:
        //   1. Pass JSON to the pipe
        //   2. Grab the output
        //   3. Redirect the output to the common receiver
        //
        //   Event reactor:
        //   - Configurable
        //   - Chain plugins/functions
        //   - Event reactions
        //   - Should probably store all the result in a common structure
        match Command::new(&self.module).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
            Ok(mut p) => {
                // Send options
                if let Some(mut stdin) = p.stdin.take() {
                    if let Err(err) = stdin.write_all(self.params_json().as_bytes()) {
                        return Err(SysinspectError::ModuleError(format!("Error while communicating with the module: {}", err)));
                    }
                }

                // Get the output
                if let Ok(out) = p.wait_with_output() {
                    match str::from_utf8(&out.stdout) {
                        Ok(out) => match serde_json::from_str::<ActionResponse>(out) {
                            Ok(r) => Ok(Some(r)),
                            Err(e) => Err(SysinspectError::ModuleError(format!("JSON error: {e}"))),
                        },
                        Err(err) => Err(SysinspectError::ModuleError(format!("Error obtaining the output: {err}"))),
                    }
                } else {
                    Err(SysinspectError::ModuleError("Module returned no output".to_string()))
                }
            }
            Err(err) => Err(SysinspectError::ModuleError(format!("Error calling module: {}", err))),
        }
    }

    pub fn state(&self) -> String {
        self.state.to_owned()
    }

    /// Get state ref
    pub fn with_state(&self, state: String) -> bool {
        self.state == state
    }
}

impl Default for ModCall {
    fn default() -> Self {
        Self { state: "$".to_string(), module: PathBuf::default(), args: Default::default(), opts: Default::default() }
    }
}

impl Display for ModCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModCall - State: {}, Module: {:?}, Opts: {:?}, Args: {:?}", self.state, self.module, self.opts, self.args)?;
        Ok(())
    }
}
