use super::response::ModResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Error;
use std::{
    collections::HashMap,
    io::{self, Read},
};

/// ArgValue is a type converter from input JSON to the internal types
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ArgValue(serde_json::Value);

impl ArgValue {
    /// Get a parameter from a comma-separated string as Vec<String>. Input example
    /// (note the space):
    ///
    /// ```
    /// "foo,bar, baz"
    /// ```
    pub fn as_strvec(&self) -> Option<Vec<String>> {
        if let Some(v) = self.as_string() {
            if !v.contains(',') {
                return None;
            }
            return Some(v.split(',').map(|s| s.trim().to_string()).collect());
        }
        None
    }

    /// Get a parameter as an integer
    pub fn as_int(&self) -> Option<i64> {
        match &self.0 {
            Value::Number(v) => Some(v.as_i64().unwrap_or_default()), // XXX: mmhhh...
            _ => None,
        }
    }

    /// Get a parameter as a bool
    pub fn as_bool(&self) -> Option<bool> {
        match &self.0 {
            Value::Bool(v) => Some(v.to_owned()),
            _ => None,
        }
    }

    /// Get a parameter as a string. Extra space is stripped.
    pub fn as_string(&self) -> Option<String> {
        match &self.0 {
            Value::String(v) => Some(v.trim().to_owned()),
            _ => None,
        }
    }
}

/// Struct to call plugin parameters
#[derive(Serialize, Deserialize, Debug)]
pub struct ModRequest {
    /// Timeout of the module running.
    /// If timeout is exceeded, module quits.
    /// 0 is no timeout.
    timeout: Option<u8>,

    /// Verbocity (quiet or logging).
    quiet: Option<bool>,

    /// Call options
    #[serde(default)]
    options: Option<Vec<ArgValue>>,

    /// Call arguments. Argumentst can have
    /// different types: list, integers, strings etc.
    #[serde(default)]
    arguments: Option<HashMap<String, ArgValue>>,

    /// Extra data, that might be needed to be passed through.
    #[serde(flatten)]
    ext: HashMap<String, serde_json::Value>,
}

impl ModRequest {
    /// Get timeout
    pub fn timeout(&self) -> u8 {
        self.timeout.unwrap_or(0).to_owned()
    }

    /// Get quiet/verbose status
    pub fn quiet(&self) -> bool {
        self.quiet.unwrap_or(false).to_owned()
    }

    /// Get param options
    pub fn options(&self) -> Vec<ArgValue> {
        self.options.to_owned().unwrap_or_default()
    }

    /// Get param args
    pub fn args(&self) -> HashMap<String, ArgValue> {
        if let Some(a) = &self.arguments {
            return a.to_owned();
        }

        HashMap::default()
    }

    /// Get optional extra data payload
    pub fn ext(&self) -> &HashMap<String, serde_json::Value> {
        &self.ext
    }
}

/// Read JSON from STDIN
pub fn get_call_args() -> Result<ModRequest, Error> {
    let mut data = String::new();
    io::stdin().read_to_string(&mut data)?;

    Ok(serde_json::from_str::<ModRequest>(&data)?)
}

/// Alias to create a `ModResponse` object
pub fn new_call_response() -> ModResponse {
    ModResponse::default()
}

/// Print JSON result to STDOUT
pub fn send_call_response(r: &ModResponse) -> Result<(), Error> {
    println!("{}", serde_json::to_string(r)?);
    Ok(())
}