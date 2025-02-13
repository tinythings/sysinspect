use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, to_value, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModResponse {
    retcode: i32,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    warning: Vec<String>,

    #[serde(skip_serializing_if = "String::is_empty")]
    message: String,

    #[serde(skip_serializing_if = "is_json_null")]
    data: Value,
}

/// Skip data inclusion into serialised output if nothing is defined
fn is_json_null(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Object(e) if e.is_empty() => true,
        Value::String(e) if e.is_empty() => true,
        Value::Array(e) if e.is_empty() => true,
        _ => false,
    }
}

impl ModResponse {
    pub fn new() -> Self {
        ModResponse::default()
    }

    /// New response with default negative/unsuccessful data for CM.
    /// These params needs to be reset.
    pub fn new_cm() -> Self {
        let mut r = ModResponse::default();
        r.set_retcode(1);
        r.set_message("Response is empty");
        _ = r.cm_set_changed(false);

        r
    }

    /// Set any payload
    pub fn set_data<T>(&mut self, data: T) -> Result<(), serde_json::Error>
    where
        T: Serialize,
    {
        //self.data = to_value(data)?;
        let v = to_value(data)?;

        if let Value::Object(ref mut map) = self.data {
            if let Value::Object(n_map) = v {
                for (k, v) in n_map {
                    map.insert(k, v);
                }
            }
        } else {
            self.data = v;
        }
        Ok(())
    }

    /// Set return message for general scope
    pub fn set_message(&mut self, text: &str) {
        self.message = text.to_string();
    }

    /// Set return code
    pub fn set_retcode(&mut self, retcode: i32) {
        self.retcode = retcode;
    }

    /// Add warning
    pub fn add_warning(&mut self, text: &str) {
        self.warning.push(text.to_string());
    }

    /// ### Confguration Management
    ///
    /// Set changed: <bool> to the data section.
    /// Used for configuration management
    pub fn cm_set_changed(&mut self, changed: bool) -> Result<(), serde_json::Error> {
        let mut out = HashMap::<String, bool>::new();
        out.insert("changed".to_string(), changed);
        self.set_data(json!(out))?;

        Ok(())
    }

    /// ### Configuration management
    ///
    /// Set STDOUT of the process
    pub fn cm_set_stdout(&mut self, data: String) -> Result<(), serde_json::Error> {
        let mut out = HashMap::<String, String>::new();
        out.insert("stdout".to_string(), data);
        self.set_data(json!(out))?;

        Ok(())
    }
}
