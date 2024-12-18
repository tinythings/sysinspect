use crate::{intp::functions::get_by_namespace, SysinspectError};
use nix::libc;
use serde::{Deserialize, Serialize};
use serde_yaml::{from_str, from_value, Value};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

// Network
pub static DEFAULT_ADDR: &str = "0.0.0.0";
pub static DEFAULT_PORT: u32 = 4200;
pub static DEFAULT_FILESERVER_PORT: u32 = 4201;

// Default directories
pub static DEFAULT_SOCKET: &str = "/var/run/sysinspect-master.socket";
pub static DEFAULT_SYSINSPECT_ROOT: &str = "/etc/sysinspect";
pub static DEFAULT_MODULES_DIR: &str = "modules";
pub static DEFAULT_PYLIB_DIR: &str = "lib";
pub static DEFAULT_SHARELIB: &str = "/usr/share/sysinspect";
pub static DEFAULT_MASTER_LOG_STD: &str = "sysmaster.standard.log";
pub static DEFAULT_MASTER_LOG_ERR: &str = "sysmaster.errors.log";
pub static DEFAULT_MINION_LOG_STD: &str = "sysminion.standard.log";
pub static DEFAULT_MINION_LOG_ERR: &str = "sysminion.errors.log";

// All directories are relative to the sysinspect root
pub static CFG_MINION_KEYS: &str = "minion-keys";
pub static CFG_MINION_REGISTRY: &str = "minion-registry";
pub static CFG_FILESERVER_ROOT: &str = "data";
pub static CFG_MODELS_ROOT: &str = "models";
pub static CFG_TRAITS_ROOT: &str = "traits";
pub static CFG_TRAIT_FUNCTIONS_ROOT: &str = "functions";
pub static CFG_DB: &str = "registry";

// Key names
pub static CFG_MASTER_KEY_PUB: &str = "master.rsa.pub";
pub static CFG_MASTER_KEY_PRI: &str = "master.rsa";
pub static CFG_MINION_RSA_PUB: &str = "minion.rsa.pub";
pub static CFG_MINION_RSA_PRV: &str = "minion.rsa";

/// Get a default location of a logfiles
fn _logfile_path() -> PathBuf {
    let mut home = String::from("");
    unsafe {
        let passwd = libc::getpwuid(libc::getuid());
        if !passwd.is_null() {
            home = std::ffi::CStr::from_ptr((*passwd).pw_dir).to_string_lossy().into_owned();
        }
    }

    for p in [format!("{home}/.local/state"), "/var/log".to_string(), "/tmp".to_string()] {
        let p = PathBuf::from(p);
        if let Ok(m) = fs::metadata(p.clone()) {
            if (m.permissions().mode() & 0o200) != 0 {
                return p;
            }
        }
    }
    PathBuf::from("")
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MinionConfig {
    /// Root directory where minion keeps all data.
    /// Default: /etc/sysinspect — same as for master
    #[serde(rename = "path.root")]
    root: Option<String>,

    /// Path to alternative /etc/machine-id
    /// Values are:
    /// - Absolute path
    /// - "relative" (keyword)
    ///
    /// Relative keyword links to $ROOT/machine-id
    #[serde(rename = "path.id")]
    machine_id: Option<String>,

    /// Path to the sharelib, other than /usr/share/sysinspect
    #[serde(rename = "path.sharelib")]
    sharelib_path: Option<String>,

    /// IP address of Master
    #[serde(rename = "master.ip")]
    master_ip: String,

    /// Port of Master. Default: 4200
    #[serde(rename = "master.port")]
    master_port: Option<u32>,

    /// Port of Master's fileserver. Default: 4201
    #[serde(rename = "master.fileserver.port")]
    master_fileserver_port: Option<u32>,

    // Standard log for daemon mode
    #[serde(rename = "log.stream")]
    log_main: Option<String>,

    // Error log for daemon mode
    #[serde(rename = "log.errors")]
    log_err: Option<String>,

    // Pidfile
    #[serde(rename = "pidfile")]
    pidfile: Option<String>,
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

    /// Get minion root directory
    pub fn root_dir(&self) -> PathBuf {
        PathBuf::from(self.root.clone().unwrap_or(DEFAULT_SYSINSPECT_ROOT.to_string()))
    }

    /// Get root directory for models
    pub fn models_dir(&self) -> PathBuf {
        self.root_dir().join(CFG_MODELS_ROOT)
    }
    /// Get root directory for functions
    pub fn functions_dir(&self) -> PathBuf {
        self.root_dir().join(CFG_TRAIT_FUNCTIONS_ROOT)
    }

    /// Get root directory for drop-in traits
    pub fn traits_dir(&self) -> PathBuf {
        self.root_dir().join(CFG_TRAITS_ROOT)
    }

    /// Return machine Id path
    pub fn machine_id_path(&self) -> PathBuf {
        if let Some(mid) = self.machine_id.clone() {
            if mid.eq("relative") {
                return self.root_dir().join("machine-id");
            } else {
                return PathBuf::from(mid);
            }
        }

        PathBuf::from("/etc/machine-id")
    }

    /// Return sharelib path
    pub fn sharelib_dir(&self) -> PathBuf {
        PathBuf::from(self.sharelib_path.clone().unwrap_or(DEFAULT_SHARELIB.to_string()))
    }

    /// Return a pidfile. Either from config or default.
    /// The default pidfile conforms to POSIX at /run/user/<ID>/....
    pub fn pidfile(&self) -> PathBuf {
        if let Some(pidfile) = &self.pidfile {
            return PathBuf::from(pidfile);
        }

        PathBuf::from(format!("/run/user/{}/sysminion.pid", unsafe { libc::getuid() }))
    }

    /// Return main logfile in daemon mode
    pub fn logfile_std(&self) -> PathBuf {
        if let Some(lfn) = &self.log_main {
            return PathBuf::from(lfn);
        }

        _logfile_path().join(DEFAULT_MINION_LOG_STD)
    }

    /// Return errors logfile in daemon mode
    pub fn logfile_err(&self) -> PathBuf {
        if let Some(lfn) = &self.log_main {
            return PathBuf::from(lfn);
        }

        _logfile_path().join(DEFAULT_MINION_LOG_ERR)
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
    fsr_ip: Option<String>,

    #[serde(rename = "fileserver.bind.port")]
    fsr_port: Option<u32>,

    // Exported models path root on the fileserver
    #[serde(rename = "fileserver.models.root")]
    fsr_models_root: String,

    // Exported models on the fileserver
    #[serde(rename = "fileserver.models")]
    fsr_models: Vec<String>,

    // Standard log for daemon mode
    #[serde(rename = "log.stream")]
    log_main: Option<String>,

    // Error log for daemon mode
    #[serde(rename = "log.errors")]
    log_err: Option<String>,

    // Pidfile
    #[serde(rename = "pidfile")]
    pidfile: Option<String>,
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
            self.fsr_ip.to_owned().unwrap_or(DEFAULT_ADDR.to_string()),
            self.fsr_port.unwrap_or(DEFAULT_FILESERVER_PORT)
        )
    }

    /// Get a list of exported models from the fileserver
    pub fn fileserver_models(&self) -> &Vec<String> {
        &self.fsr_models
    }

    /// Get fileserver root
    pub fn fileserver_root(&self) -> PathBuf {
        self.root_dir().join(CFG_FILESERVER_ROOT)
    }

    /// Get models root on the fileserver
    pub fn fileserver_mdl_root(&self, alone: bool) -> PathBuf {
        let mr = PathBuf::from(&self.fsr_models_root.strip_prefix("/").unwrap_or_default());
        if alone {
            mr
        } else {
            self.fileserver_root().join(mr)
        }
    }

    /// Get default sysinspect root. For master it is always /etc/sysinspect
    pub fn root_dir(&self) -> PathBuf {
        PathBuf::from(DEFAULT_SYSINSPECT_ROOT.to_string())
    }

    /// Get minion keys store
    pub fn keyman_root(&self) -> PathBuf {
        self.root_dir().join(CFG_MINION_KEYS)
    }

    /// Get minion registry
    pub fn minion_registry_root(&self) -> PathBuf {
        self.root_dir().join(CFG_MINION_REGISTRY)
    }

    /// Return a pidfile. Either from config or default.
    /// The default pidfile conforms to POSIX at /run/user/<ID>/....
    pub fn pidfile(&self) -> PathBuf {
        if let Some(pidfile) = &self.pidfile {
            return PathBuf::from(pidfile);
        }

        PathBuf::from(format!("/run/user/{}/sysmaster.pid", unsafe { libc::getuid() }))
    }

    /// Return main logfile in daemon mode
    pub fn logfile_std(&self) -> PathBuf {
        if let Some(lfn) = &self.log_main {
            return PathBuf::from(lfn);
        }

        _logfile_path().join(DEFAULT_MASTER_LOG_STD)
    }

    /// Return errors logfile in daemon mode
    pub fn logfile_err(&self) -> PathBuf {
        if let Some(lfn) = &self.log_main {
            return PathBuf::from(lfn);
        }

        _logfile_path().join(DEFAULT_MASTER_LOG_ERR)
    }
}
