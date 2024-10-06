use serde::{Deserialize, Serialize};
use serde_json::to_value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModResponse {
    retcode: i32,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    warning: Vec<String>,

    #[serde(skip_serializing_if = "String::is_empty")]
    message: String,

    #[serde(skip_serializing_if = "is_json_null")]
    data: serde_json::Value,
}

/// Skip data inclusion into serialised output if nothing is defined
fn is_json_null(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => true,
        serde_json::Value::Object(e) if e.is_empty() => todo!(),
        serde_json::Value::String(e) if e.is_empty() => true,
        serde_json::Value::Array(e) if e.is_empty() => true,
        _ => false,
    }
}

impl ModResponse {
    pub fn new() -> Self {
        ModResponse::default()
    }

    /// Set any payload
    pub fn set_data<T>(mut self, data: T) -> Result<Self, serde_json::Error>
    where
        T: Serialize,
    {
        self.data = to_value(data)?;
        Ok(self)
    }

    /// Set return message for general scope
    pub fn set_message(mut self, text: String) -> Self {
        self.message = text;
        self
    }

    /// Set return code
    pub fn set_retcode(mut self, retcode: i32) -> Self {
        self.retcode = retcode;
        self
    }

    /// Add warning
    pub fn add_warning(mut self, text: String) -> Self {
        self.warning.push(text);
        self
    }
}
