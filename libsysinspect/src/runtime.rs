use serde::{Deserialize, Serialize};
use std::io::Error;
use std::{
    collections::HashMap,
    io::{self, Read},
};

#[derive(Serialize, Deserialize, Debug)]
pub struct PluginResponse {
    /// General main response message
    info: String,

    /// Log messages (whatever a plugin wants to pass-through)
    messages: Vec<String>,

    /// General return status
    return_status: bool,

    /// General return code, if any.
    return_code: i8,
}

impl PluginResponse {
    pub fn new(info: String) -> Self {
        PluginResponse {
            info,
            messages: vec![],

            // Return status is success (true)
            return_status: true,

            // Return code is success (0)
            return_code: 0,
        }
    }

    /// Set general return status
    pub fn set_status(&mut self, status: bool) -> &mut Self {
        self.return_status = status;
        self
    }

    /// Set general return code
    pub fn set_code(&mut self, code: i8) -> &mut Self {
        self.return_code = code;
        self
    }
}

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

// Print JSON result to STDOUT
pub fn send_call_response(r: &PluginResponse) -> Result<(), Error> {
    println!("{}", serde_json::to_string(r)?);
    Ok(())
}