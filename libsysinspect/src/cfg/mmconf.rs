use crate::{intp::functions::get_by_namespace, SysinspectError};
use serde::{Deserialize, Serialize};
use serde_yaml::{from_str, from_value, Value};
use std::{fs, path::PathBuf};

static DEFAULT_ADDR: &str = "0.0.0.0";
static DEFAULT_PORT: u32 = 4200;
static DEFAULT_FILESERVER_PORT: u32 = 4201;
static DEFAULT_SOCKET: &str = "/var/run/sysinspect-master.socket";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MinionConfig {
    #[serde(rename = "master.ip")]
    master_ip: String,

    #[serde(rename = "master.port")]
    master_port: Option<u32>,

    #[serde(rename = "master.fileserver.port")]
    master_fileserver_port: Option<u32>,
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
        format!("{}:{}", self.master_ip, self.master_port.unwrap_or(DEFAULT_PORT))
    }

    /// Return master fileserver addr
    pub fn fileserver(&self) -> String {
        format!("{}:{}", self.master_ip, self.master_fileserver_port.unwrap_or(DEFAULT_FILESERVER_PORT))
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MasterConfig {
    // Bind IP listener. Default "the world", i.e. 0.0.0.0
    #[serde(rename = "bind.ip")]
    bind_ip: Option<String>,

    // Bind port. Default 4200
    #[serde(rename = "bind.port")]
    bind_port: Option<u32>,

    // Path to FIFO socket. Default: /var/run/sysinspect-master.socket
    socket: Option<String>,

    #[serde(rename = "fileserver.bind.ip")]
    fileserver_ip: Option<String>,

    #[serde(rename = "fileserver.bind.port")]
    fileserver_port: Option<u32>,
}

impl MasterConfig {
    pub fn new(p: PathBuf) -> Result<MasterConfig, SysinspectError> {
        let cp = p.as_os_str().to_str().unwrap_or_default();
        if !p.exists() {
            return Err(SysinspectError::ConfigError(format!("File not found: {}", cp)));
        }

        if let Some(cfgv) = get_by_namespace(Some(from_str::<Value>(&fs::read_to_string(&p)?)?), "config.master") {
            return Ok(from_value::<MasterConfig>(cfgv)?);
        }

        Err(SysinspectError::ConfigError(format!("Unable to read config at: {}", cp)))
    }

    /// Return master addr
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.bind_ip.to_owned().unwrap_or(DEFAULT_ADDR.to_string()), self.bind_port.unwrap_or(DEFAULT_PORT))
    }

    /// Get socket address
    pub fn socket(&self) -> String {
        self.socket.to_owned().unwrap_or(DEFAULT_SOCKET.to_string())
    }

    /// Return fileserver addr
    pub fn fileserver_bind_addr(&self) -> String {
        format!(
            "{}:{}",
            self.fileserver_ip.to_owned().unwrap_or(DEFAULT_ADDR.to_string()),
            self.fileserver_port.unwrap_or(DEFAULT_FILESERVER_PORT)
        )
    }
}
