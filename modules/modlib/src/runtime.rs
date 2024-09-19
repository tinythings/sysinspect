use serde::{Deserialize, Serialize};
use std::io::Error;
use std::{
    collections::HashMap,
    io::{self, BufRead, Read},
};

// Struct to call plugin parameters
#[derive(Serialize, Deserialize, Debug)]
pub struct PluginParams {
    /// Timeout of the module running.
    /// If timeout is exceeded, module quits.
    /// 0 is no timeout.
    timeout: Option<u8>,

    /// Verbocity (quiet or logging).
    quiet: Option<bool>,

    /// Call options
    #[serde(default)]
    options: Option<Vec<String>>,

    /// Call arguments. Argumentst can have
    /// different types: list, integers, strings etc.
    #[serde(default)]
    args: Option<HashMap<String, serde_json::Value>>,

    /// Extra data, that might be needed to be passed through.
    #[serde(flatten)]
    ext: HashMap<String, serde_json::Value>,
}

impl PluginParams {
    /// Get timeout
    pub fn timeout(&self) -> u8 {
        if let Some(timeout) = self.timeout {
            return timeout;
        }

        0
    }

    /// Get quiet/verbose status
    pub fn quiet(&self) -> bool {
        if let Some(q) = self.quiet {
            q
        } else {
            false
        }
    }

    /// Get param options
    pub fn options(&self) -> Vec<String> {
        if let Some(o) = &self.options {
            return o.clone();
        }

        vec![]
    }

    /// Get param args
    pub fn args(&self) -> HashMap<String, serde_json::Value> {
        if let Some(a) = &self.args {
            return a.clone();
        }

        HashMap::new()
    }

    /// Get optional extra data payload
    pub fn ext(&self) -> &HashMap<String, serde_json::Value> {
        &self.ext
    }
}

// Read JSON from STDIN
pub fn get_call_args() -> Result<PluginParams, Error> {
    let mut data = String::new();
    io::stdin().read_to_string(&mut data)?;

    Ok(serde_json::from_str::<PluginParams>(&data)?)
}
