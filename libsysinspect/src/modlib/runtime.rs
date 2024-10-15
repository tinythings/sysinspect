use super::response::ModResponse;
use crate::util;
use serde::{Deserialize, Serialize};
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
    pub fn as_str_vec(&self) -> Option<Vec<String>> {
        util::dataconv::as_str_list_opt(Some(&self.0).cloned())
    }

    /// Get a parameter as an integer
    pub fn as_int(&self) -> Option<i64> {
        util::dataconv::as_int_opt(Some(&self.0).cloned())
    }

    /// Get a parameter as a bool
    pub fn as_bool(&self) -> Option<bool> {
        util::dataconv::as_bool_opt(Some(&self.0).cloned())
    }

    /// Get a parameter as a string. Extra space is stripped.
    pub fn as_string(&self) -> Option<String> {
        util::dataconv::as_str_opt(Some(&self.0).cloned())
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
    #[serde(alias = "opts")]
    options: Option<Vec<ArgValue>>,

    /// Call arguments. Argumentst can have
    /// different types: list, integers, strings etc.
    #[serde(default)]
    #[serde(alias = "args")]
    arguments: Option<Vec<HashMap<String, ArgValue>>>,

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

    /// Get all param args
    pub fn args(&self) -> Vec<HashMap<String, ArgValue>> {
        if let Some(a) = &self.arguments {
            return a.to_owned();
        }

        Vec::default()
    }

    /// Short-cut to get a first argument (usually it is)
    pub fn first_arg(&self, kw: &str) -> Option<ArgValue> {
        if let Some(a) = &self.arguments {
            for a in a {
                if let Some(a) = a.get(kw) {
                    return Some(a.to_owned());
                }
            }
        };

        None
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

/// Get a string argument
pub fn get_arg(rt: &ModRequest, arg: &str) -> String {
    if let Some(s_arg) = rt.first_arg(arg) {
        if let Some(s_arg) = s_arg.as_string() {
            return s_arg;
        } else if let Some(s_arg) = s_arg.as_bool() {
            return format!("{}", s_arg);
        }
    }
    "".to_string()
}

/// Get a presence of a flag/option
pub fn get_opt(rt: &ModRequest, opt: &str) -> bool {
    for av in rt.options() {
        if av.as_string().unwrap_or_default().eq(opt) {
            return true;
        }
    }
    false
}
