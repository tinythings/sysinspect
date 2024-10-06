use serde::{Deserialize, Serialize};
use serde_json::to_value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModResponse {
    retcode: i32,
    warning: Vec<String>,
    message: String,
    data: serde_json::Value,
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
