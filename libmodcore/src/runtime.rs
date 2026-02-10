use crate::rtspec::RuntimeParams;

use super::response::ModResponse;
use indexmap::IndexMap;
use libsysinspect::cfg::mmconf::DEFAULT_MODULES_SHARELIB;
use libsysinspect::util;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Error;
use std::io::{self, Read};

/// ArgValue is a type converter from input JSON to the internal types
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
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

impl From<ArgValue> for serde_json::Value {
    fn from(val: ArgValue) -> Self {
        val.0
    }
}

/// Struct to call plugin parameters
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
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
    arguments: Option<IndexMap<String, ArgValue>>,

    /// Passed-through MinionConfig (only defined parts)
    /// If nothing defined at all, use default constants.
    #[serde(default)]
    config: Option<IndexMap<String, ArgValue>>,

    /// Extra data, that might be needed to be passed through.
    #[serde(flatten)]
    ext: IndexMap<String, serde_json::Value>,
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

    pub fn options_all(&self) -> Vec<ArgValue> {
        self.options.to_owned().unwrap_or_default()
    }

    /// Get param options
    pub fn options(&self) -> Vec<ArgValue> {
        let mut out = Vec::new();
        for av in self.options.to_owned().unwrap_or_default() {
            if let Some(s) = av.as_string()
                && !s.starts_with(&RuntimeParams::RtPrefix.to_string())
            {
                out.push(av);
            }
        }
        out
    }

    /// Check if an option is present
    pub fn has_option(&self, opt: &str) -> bool {
        for av in self.options_all() {
            if av.as_string().unwrap_or_default().eq(opt) {
                return true;
            }
        }
        false
    }

    pub fn config(&self) -> IndexMap<String, ArgValue> {
        // Inject sharelib path if not defined
        // Modules not supposed to take explicit care where is their shared library located,
        // but simply read the configuration. For example, runtimes need to know where to find their
        // modules.
        let mut config = self.config.clone().unwrap_or_default();
        if config.get("path.sharelib").is_none() {
            config.insert("path.sharelib".to_string(), ArgValue(serde_json::Value::String(DEFAULT_MODULES_SHARELIB.to_string())));
        }
        config
    }

    /// Get all param args including runtime-specific ones (those starting with "rt.")
    pub fn args_all(&self) -> IndexMap<String, ArgValue> {
        self.arguments.clone().unwrap_or_default()
    }

    /// Get all param args without runtime-specific ones (those starting with "rt.")
    pub fn args(&self) -> IndexMap<String, ArgValue> {
        let mut target_args = IndexMap::new();
        for (k, v) in self.arguments.clone().unwrap_or_default() {
            if !k.starts_with(&RuntimeParams::RtPrefix.to_string()) {
                target_args.insert(k, v);
            }
        }
        target_args
    }

    /// Get arg
    pub fn get_arg(&self, kw: &str) -> Option<ArgValue> {
        if let Some(a) = &self.arguments
            && let Some(a) = a.get(kw)
        {
            return Some(a.clone());
        };

        None
    }

    /// Get optional extra data payload
    pub fn ext(&self) -> &IndexMap<String, serde_json::Value> {
        &self.ext
    }

    /// Add an option
    ///
    /// This method typically used to alter a ModRequest by runtimes and
    /// not supposed to be used within modules realm.
    pub fn add_opt(&mut self, arg: &str) {
        let arg = ArgValue(serde_json::Value::String(arg.to_string()));
        let mut opts = self.options.clone().unwrap_or_default();
        opts.push(arg);
        self.options = Some(opts);
    }

    /// Add an argument
    ///
    /// This method typically used to alter a ModRequest by runtimes and
    /// not supposed to be used within modules realm.
    pub fn add_arg(&mut self, key: &str, val: Value) {
        let mut args = self.arguments.clone().unwrap_or_default();
        args.insert(key.to_string(), ArgValue(val));
        self.arguments = Some(args);
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
    if let Some(s_arg) = rt.get_arg(arg) {
        if let Some(s_arg) = s_arg.as_string() {
            return s_arg;
        } else if let Some(s_arg) = s_arg.as_bool() {
            return format!("{s_arg}");
        }
    }
    "".to_string()
}

/// Get a string argument with default value
pub fn get_arg_default(rt: &ModRequest, arg: &str, default: &str) -> String {
    let s_arg = get_arg(rt, arg);
    if s_arg.is_empty() { default.to_string() } else { s_arg }
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
