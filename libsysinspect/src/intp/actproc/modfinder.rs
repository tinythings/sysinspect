use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, path::PathBuf};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModCall {
    state: String,
    module: PathBuf,
    args: Vec<HashMap<String, String>>,
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
        self.args.push([(kw, arg)].into_iter().collect());
        self
    }

    /// Add an option
    pub fn add_opt(&mut self, opt: String) -> &mut Self {
        self.opts.push(opt);
        self
    }

    pub fn run(&self) {
        log::debug!("run() of {}", self);
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
