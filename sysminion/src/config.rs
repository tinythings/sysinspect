use libsysinspect::{intp::functions::get_by_namespace, SysinspectError};
use serde::{Deserialize, Serialize};
use serde_yaml::{from_str, from_value, Value};
use std::{fs, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MinionConfig {
    #[serde(rename = "master.ip")]
    master_ip: String,

    #[serde(rename = "master.port")]
    master_port: u32,
}

impl MinionConfig {
    pub fn new(p: PathBuf) -> Result<MinionConfig, SysinspectError> {
        let cp = p.as_os_str().to_str().unwrap_or_default();
        if !p.exists() {
            return Err(SysinspectError::ConfigError(format!("File not found: {}", cp)));
        }

        if let Some(cfgv) = get_by_namespace(Some(from_str::<Value>(&fs::read_to_string(&p)?)?), "config.minion") {
            return Ok(from_value::<MinionConfig>(cfgv)?);
        }

        Err(SysinspectError::ConfigError(format!("Unable to read config at: {}", cp)))
    }

    /// Return master addr
    pub fn master(&self) -> String {
        format!("{}:{}", self.master_ip, self.master_port)
    }
}
