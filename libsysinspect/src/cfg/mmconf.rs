use crate::{SysinspectError, intp::functions::get_by_namespace, util};
use indexmap::IndexMap;
use nix::libc;
use serde::{Deserialize, Serialize};
use serde_yaml::{Value, from_str, from_value};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

// Network
// -------

/// Default listener address (to the world)
pub static DEFAULT_ADDR: &str = "0.0.0.0";

/// Default listener port communication protocol for the master
pub static DEFAULT_PORT: u32 = 4200;

/// Default fileserver port on the master
pub static DEFAULT_FILESERVER_PORT: u32 = 4201;

/// Default API port for the web API
pub static DEFAULT_API_PORT: u32 = 4202;

// Default directories
// --------------------

/// Location of the communication socket between **sysinspect controller**
/// and the running **master daemon**.
pub static DEFAULT_SOCKET: &str = "/var/run/sysinspect-master.socket";

/// The main root of the sysinspect data, both for **master** and **minion**,
/// also for the command controller.
pub static DEFAULT_SYSINSPECT_ROOT: &str = "/etc/sysinspect";

/// Default location of all modules and libraries for them,
/// including Python stack as well. Refer to other variables with
/// `DEFAULT_MODULES_*` prefix.
pub static DEFAULT_MODULES_SHARELIB: &str = "/usr/share/sysinspect";

/// Directory within the `DEFAULT_MODULES_SHARELIB` for modules
pub static DEFAULT_MODULES_DIR: &str = "modules";

/// Directory within the `DEFAULT_MODULES_SHARELIB` for python libraries
pub static DEFAULT_MODULES_LIB_DIR: &str = "lib";

/// Default filename for the master log
pub static DEFAULT_MASTER_LOG_STD: &str = "sysmaster.standard.log";

/// Default filename for the master failures log
pub static DEFAULT_MASTER_LOG_ERR: &str = "sysmaster.errors.log";

/// Default path for the telemetry database
pub static DEFAULT_MASTER_TELEMETRY_DB: &str = "/var/tmp/sysinspect/telemetry";

/// Default path for the telemetry communication socket
pub static DEFAULT_MASTER_TELEMETRY_SCK: &str = "/tmp/sysinspect.telemetry.sock";

/// Default filename for the minion log
pub static DEFAULT_MINION_LOG_STD: &str = "sysminion.standard.log";

/// Default filename for the minion failures log
pub static DEFAULT_MINION_LOG_ERR: &str = "sysminion.errors.log";

// All directories are relative to the sysinspect root
// ---------------------------------------------------
pub static CFG_MINION_KEYS: &str = "minion-keys";
pub static CFG_MINION_REGISTRY: &str = "minion-registry";
pub static CFG_API_KEYS: &str = "webapi-keys";
pub static CFG_FILESERVER_ROOT: &str = "data";
pub static CFG_DB: &str = "registry";

// Repository for modules within the CFG_FILESERVER_ROOT
pub static CFG_MODREPO_ROOT: &str = "repo";

// Served models within the CFG_FILESERVER_ROOT
pub static CFG_MODELS_ROOT: &str = "models";

// Traits within the CFG_FILESERVER_ROOT
pub static CFG_TRAITS_ROOT: &str = "traits";

// Trait custom functions within the CFG_FILESERVER_ROOT
pub static CFG_TRAIT_FUNCTIONS_ROOT: &str = "functions";

// Key names
// ---------
pub static CFG_MASTER_KEY_PUB: &str = "master.rsa.pub";
pub static CFG_MASTER_KEY_PRI: &str = "master.rsa";
pub static CFG_MINION_RSA_PUB: &str = "minion.rsa.pub";
pub static CFG_MINION_RSA_PRV: &str = "minion.rsa";

// Sync
// ----
pub static CFG_AUTOSYNC_FULL: &str = "full";
pub static CFG_AUTOSYNC_FAST: &str = "fast";
pub static CFG_AUTOSYNC_SHALLOW: &str = "shallow";
pub static CFG_AUTOSYNC_DEFAULT: &str = CFG_AUTOSYNC_FULL;

// Reconnect to the master
pub static CFG_RECONNECT_DEFAULT: bool = true;
pub static CFG_RECONNECT_FREQ_DEFAULT: u32 = 0;
pub static CFG_RECONNECT_DEFAULT_INTERVAL: &str = "5-30";

// Task Intervals
// ---------------
pub const CFG_TASK_INTERVAL_SECONDS: &str = "seconds";
pub const CFG_TASK_INTERVAL_MINUTES: &str = "minutes";
pub const CFG_TASK_INTERVAL_HOURS: &str = "hours";
pub const CFG_TASK_INTERVAL_DAYS: &str = "days";

// Telemetry (OTLP)
pub static CFG_OTLP_COLLECTOR: &str = "127.0.0.1:4317"; // Default collector address
pub static CFG_OTLP_SERVICE_NAME: &str = "sysinspect";
pub static CFG_OTLP_SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub static CFG_OTLP_COMPRESSION: &str = "gzip"; // or "zstd"

// History keeping
pub static CFG_HISTORY_LIMIT: usize = 100;
pub static CFG_HISTORY_AGE: usize = 86400; // 1 day in seconds

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum AuthMethod {
    /// Use PAM authentication
    Pam,

    /// Use custom authentication method, e.g. LDAP
    Ldap,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct HistoryConfig {
    // Max number of history records to keep
    limit: Option<usize>,

    // Max age in units of seconds, minutes, hours, or days
    #[serde(default)]
    #[serde(with = "humantime_serde::option")]
    age: Option<Duration>,

    // Rotate history records
    rotate: Option<bool>,
}

impl HistoryConfig {
    /// Get max number of history records to keep
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(CFG_HISTORY_LIMIT)
    }

    /// Get max age in seconds, minutes, hours, or days
    pub fn age(&self) -> Duration {
        self.age.unwrap_or_else(|| Duration::from_secs(CFG_HISTORY_AGE as u64))
    }

    /// Rotate history records
    pub fn rotate(&self) -> bool {
        self.rotate.unwrap_or(false)
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TelemetryConfig {
    #[serde(rename = "collector.grpc")]
    collector: Option<String>, // Default localhost

    #[serde(rename = "collector.compression")]
    compression: Option<String>,

    #[serde(rename = "exporter-resources")]
    exporter_resources: Option<IndexMap<String, Value>>,

    #[serde(rename = "exporter-scope")]
    exporter_scope: Option<IndexMap<String, String>>,
}

impl TelemetryConfig {
    /// Get collector address
    pub fn collector(&self) -> String {
        self.collector.clone().unwrap_or(CFG_OTLP_COLLECTOR.to_string())
    }

    /// Get compression mode
    pub fn compression(&self) -> String {
        self.compression.clone().unwrap_or(CFG_OTLP_COMPRESSION.to_string())
    }

    /// Get exporter resources
    pub fn resources(&self) -> IndexMap<String, String> {
        let mut resources = IndexMap::new();
        let mut skipped = vec![];
        for (key, value) in self.exporter_resources.clone().unwrap_or_default() {
            if let Some(v) = value.as_bool() {
                if !v {
                    skipped.push(key);
                    continue;
                }
            }

            let value = util::dataconv::to_string(Some(value)).unwrap_or_default().trim().to_string();
            if value.is_empty() {
                continue;
            }

            resources.insert(key, value);
        }

        for (k, v) in vec![
            ("service.name", CFG_OTLP_SERVICE_NAME),
            ("service.namespace", CFG_OTLP_SERVICE_NAME),
            ("service.version", CFG_OTLP_SERVICE_VERSION),
            ("host.name", sysinfo::System::host_name().unwrap_or_default().as_str()),
            ("os.type", sysinfo::System::distribution_id().as_str()),
            ("deployment.environment", "production"),
            ("os.version", sysinfo::System::kernel_version().unwrap_or_default().as_str()),
        ] {
            if !resources.contains_key(k) && !skipped.contains(&k.to_string()) {
                resources.insert(k.to_string(), v.to_string());
            }
        }

        resources
    }

    /// Get exporter scope
    pub fn scope(&self) -> IndexMap<String, String> {
        let mut scope = IndexMap::new();
        for (k, v) in self.exporter_scope.clone().unwrap_or_default() {
            scope.insert(k, v);
        }

        // XXX: Fix the model name getting from an actual query
        for (k, v) in [("name", "model")] {
            if !scope.contains_key(k) {
                scope.insert(k.to_string(), v.to_string());
            }
        }

        scope
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TaskConfig {
    name: String,
    query: String,
    traits: Option<Vec<String>>,
    interval: u32,
    #[serde(rename = "interval.unit")]
    interval_unit: String,
}

impl TaskConfig {
    /// Create a new task
    pub fn new(name: &str, query: &str, interval: u32, unit: &str) -> Self {
        Self { name: name.to_string(), query: query.to_string(), traits: None, interval, interval_unit: unit.to_string() }
    }

    /// Set task traits
    pub fn traits(&self) -> Option<&Vec<String>> {
        self.traits.as_ref()
    }

    /// Get task name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get query (path, scope, traits, minion id)
    pub fn query(&self) -> String {
        let mut p: Vec<&str> = self.query.splitn(4, ';').collect();
        while p.len() < 4 {
            p.push("");
        }
        p.join(";")
    }

    /// Get interval in seconds
    pub fn interval(&self) -> (u32, String) {
        (self.interval, self.interval_unit.clone())
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct MinionConfig {
    /// Root directory where minion keeps all data.
    /// Default: /etc/sysinspect — same as for master
    #[serde(rename = "path.root")]
    #[serde(skip_serializing_if = "Option::is_none")]
    root: Option<String>,

    /// Path to alternative /etc/machine-id
    /// Values are:
    /// - Absolute path
    /// - "relative" (keyword)
    ///
    /// Relative keyword links to $ROOT/machine-id
    #[serde(rename = "path.id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    machine_id: Option<String>,

    /// Path to the sharelib, other than /usr/share/sysinspect
    #[serde(rename = "path.sharelib")]
    #[serde(skip_serializing_if = "Option::is_none")]
    sharelib_path: Option<String>,

    /// Check module checksup on startup. It has three values:
    /// - full: calculate the checksum of each module
    /// - fast: compare the checksum of each module with the one stored in the cache
    /// - shallow: verify only if the file exists. NOTE: this does not defend against the local minion changes!
    /// - Any other value: defaults to `full` behavior.
    ///
    /// Default: full
    #[serde(rename = "modules.autosync")]
    #[serde(skip_serializing_if = "Option::is_none")]
    modules_check: Option<String>,

    /// IP address of Master
    #[serde(rename = "master.ip")]
    #[serde(skip_serializing_if = "String::is_empty")]
    master_ip: String,

    /// Port of Master. Default: 4200
    #[serde(rename = "master.port")]
    #[serde(skip_serializing_if = "Option::is_none")]
    master_port: Option<u32>,

    /// Port of Master's fileserver. Default: 4201
    #[serde(rename = "master.fileserver.port")]
    #[serde(skip_serializing_if = "Option::is_none")]
    master_fileserver_port: Option<u32>,

    /// Reconnect policies to the master
    /// Values are:
    /// - "true" (default)
    /// - "false"
    #[serde(rename = "master.reconnect")]
    #[serde(skip_serializing_if = "Option::is_none")]
    master_reconnect: Option<bool>,

    /// Reconnect frequencies to the master
    /// Values are:
    /// - 0 is infinite (default)
    /// - More than 0
    #[serde(rename = "master.reconnect.freq")]
    #[serde(skip_serializing_if = "Option::is_none")]
    master_reconnect_freq: Option<u32>,

    /// Reconnect interval in seconds
    /// Values are:
    /// - n is a number of seconds
    /// - n-n range is a random number of seconds within the range
    #[serde(rename = "master.reconnect.interval")]
    #[serde(skip_serializing_if = "Option::is_none")]
    master_reconnect_interval: Option<String>,

    // Standard log for daemon mode
    #[serde(rename = "log.stream")]
    #[serde(skip_serializing_if = "Option::is_none")]
    log_main: Option<String>,

    // Error log for daemon mode
    #[serde(rename = "log.errors")]
    #[serde(skip_serializing_if = "Option::is_none")]
    log_err: Option<String>,

    // Pidfile
    #[serde(rename = "pidfile")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pidfile: Option<String>,
}

impl MinionConfig {
    pub fn new(p: PathBuf) -> Result<MinionConfig, SysinspectError> {
        let cp = p.as_os_str().to_str().unwrap_or_default();
        if !p.exists() {
            return Err(SysinspectError::ConfigError(format!("File not found: {cp}")));
        }

        if let Some(cfgv) = get_by_namespace(Some(from_str::<Value>(&fs::read_to_string(&p)?)?), "config.minion") {
            return Ok(from_value::<MinionConfig>(cfgv)?);
        }

        Err(SysinspectError::ConfigError(format!("Unable to read config at: {cp}")))
    }

    /// Set Master IP
    pub fn set_master_ip(&mut self, ip: &str) {
        if ip.is_empty() {
            return;
        }

        self.master_ip = ip.to_string();
    }

    /// Set Master fileserver port
    pub fn set_master_port(&mut self, port: u32) {
        self.master_port = Some(port);
    }

    /// Set root directory
    pub fn set_root_dir(&mut self, dir: &str) {
        self.root = Some(dir.to_string());
    }

    /// Set sharelib path
    pub fn set_sharelib_path(&mut self, p: &str) {
        self.sharelib_path = Some(p.to_string());
    }

    /// Set pidfile path
    pub fn set_pid_path(&mut self, p: &str) {
        self.pidfile = Some(p.to_string());
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
        PathBuf::from(self.sharelib_path.clone().unwrap_or(DEFAULT_MODULES_SHARELIB.to_string()))
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

    /// Return modules.fastsync flag
    pub fn autosync(&self) -> String {
        self.modules_check.as_ref().unwrap_or(&CFG_AUTOSYNC_DEFAULT.to_string()).clone()
    }

    /// Set autosync mode
    pub fn set_autosync(&mut self, mode: &str) {
        self.modules_check = Some(mode.to_string());
    }

    /// Set reconnection policy to the master
    pub fn set_reconnect_freq(&mut self, freq: u32) {
        self.master_reconnect_freq = Some(freq);
    }

    /// Set reconnection interval to the master
    pub fn set_reconnect_interval(&mut self, interval: &str) {
        self.master_reconnect_interval = Some(interval.to_string());
    }

    /// Reconnect policy
    pub fn reconnect(&self) -> bool {
        self.master_reconnect.unwrap_or(CFG_RECONNECT_DEFAULT)
    }

    /// Reconnect frequencies
    pub fn reconnect_freq(&self) -> u32 {
        self.master_reconnect_freq.unwrap_or(CFG_RECONNECT_FREQ_DEFAULT)
    }

    /// Reconnect interval (seconds)
    pub fn reconnect_interval(&self) -> u64 {
        let i = self.master_reconnect_interval.clone().unwrap_or(CFG_RECONNECT_DEFAULT_INTERVAL.to_string());
        if let Ok(i) = i.parse::<u64>() {
            return i;
        }
        if let Some((start, end)) = i.split_once('-') {
            if let (Ok(start), Ok(end)) = (start.parse::<u64>(), end.parse::<u64>()) {
                return if end > start { rand::random::<u64>() % (end - start + 1) + start } else { start };
            }
        }

        rand::random::<u64>() % 30 + 5
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

    #[serde(rename = "api.enabled")]
    api_enabled: Option<bool>,

    #[serde(rename = "api.bind.ip")]
    api_ip: Option<String>,

    #[serde(rename = "api.bind.port")]
    api_port: Option<u32>,

    #[serde(rename = "api.version")]
    api_version: Option<u8>,

    // "pam" or any other in a future, e.g. "ldap"
    #[serde(rename = "api.auth")]
    pam_enabled: Option<String>,

    /// Disable libsodium crypto and authentication.
    /// Still need auth, but can be just empty strings passed
    /// and development mode token used.
    ///
    /// WARNING: **DO NOT USE IN PRODUCTION! IT FULLY DISABLES ENCRYPTION!!!**
    #[serde(rename = "api.devmode")]
    dev_mode: Option<bool>,

    // Standard log for daemon mode
    #[serde(rename = "log.stream")]
    log_main: Option<String>,

    // Error log for daemon mode
    #[serde(rename = "log.errors")]
    log_err: Option<String>,

    // Pidfile
    #[serde(rename = "pidfile")]
    pidfile: Option<String>,

    // Telemetry database location (local key/value storage)
    #[serde(rename = "telemetry.location")]
    telemetry_location: Option<String>,

    // Telemetry socket communication between
    // sysinspect and sysmaster
    #[serde(rename = "telemetry.socket")]
    telemetry_socket: Option<String>,

    // OpenTelemetry (OTLP) configuration
    telemetry: Option<TelemetryConfig>,

    // Scheduler for recurring queries to all the minions
    scheduler: Option<Vec<TaskConfig>>,

    // Configuration of history keeping in the database per each cycle call
    history: Option<HistoryConfig>,
}

impl MasterConfig {
    pub fn new(p: PathBuf) -> Result<MasterConfig, SysinspectError> {
        let cp = p.as_os_str().to_str().unwrap_or_default();
        if !p.exists() {
            return Err(SysinspectError::ConfigError(format!("File not found: {cp}")));
        }

        if let Some(cfgv) = get_by_namespace(Some(from_str::<Value>(&fs::read_to_string(&p)?)?), "config.master") {
            return Ok(from_value::<MasterConfig>(cfgv)?);
        }

        Err(SysinspectError::ConfigError(format!("Unable to read config at: {cp}")))
    }

    /// Get OTLP collector endpoint
    pub fn otlp_collector_endpoint(&self) -> String {
        let mut uri = String::new();
        if let Some(cfg) = &self.telemetry {
            uri = cfg.collector.clone().unwrap_or(CFG_OTLP_COLLECTOR.to_string());
        }
        format!("http://{}", uri.split("://").last().unwrap_or(&uri))
    }

    /// Get OTLP compression mode. Usually should be default.
    pub fn otlp_compression(&self) -> String {
        let mut cpr = CFG_OTLP_COMPRESSION.to_string();
        if let Some(cfg) = &self.telemetry {
            if let Some(compression) = &cfg.compression {
                cpr = compression.clone();
            }
        }

        if !cpr.eq("gzip") && !cpr.eq("zstd") {
            return CFG_OTLP_COMPRESSION.to_string();
        }

        cpr
    }

    /// Get history configuration
    pub fn history(&self) -> HistoryConfig {
        if let Some(cfg) = &self.history {
            return cfg.clone();
        }

        HistoryConfig::default()
    }

    /// Get OTLP configuration
    pub fn otlp_cfg(&self) -> TelemetryConfig {
        if let Some(cfg) = &self.telemetry {
            return cfg.clone();
        }

        TelemetryConfig::default()
    }

    /// Get scheduler tasks
    pub fn scheduler(&self) -> Vec<TaskConfig> {
        self.scheduler.clone().unwrap_or_default()
    }

    /// Return master addr
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.bind_ip.to_owned().unwrap_or(DEFAULT_ADDR.to_string()), self.bind_port.unwrap_or(DEFAULT_PORT))
    }

    /// Get socket address
    pub fn socket(&self) -> String {
        self.socket.to_owned().unwrap_or(DEFAULT_SOCKET.to_string())
    }

    /// Get API enabled status (default: true)
    pub fn api_enabled(&self) -> bool {
        self.api_enabled.unwrap_or(true)
    }

    /// Get API bind IP address
    pub fn api_bind_addr(&self) -> String {
        self.api_ip.to_owned().unwrap_or(DEFAULT_ADDR.to_string())
    }

    /// Get API bind port
    pub fn api_bind_port(&self) -> u32 {
        self.api_port.unwrap_or(DEFAULT_API_PORT)
    }

    /// Get API version
    pub fn api_version(&self) -> u8 {
        self.api_version.unwrap_or(1)
    }

    /// Get API authentication method
    pub fn api_auth(&self) -> AuthMethod {
        match self.pam_enabled.as_deref().map(|s| s.to_ascii_lowercase()) {
            Some(ref s) if s == "pam" => AuthMethod::Pam,
            Some(ref s) if s == "ldap" => AuthMethod::Ldap,
            Some(_) | None => AuthMethod::Pam,
        }
    }

    /// Get API development mode
    /// This is a special mode for development purposes only.
    /// It disables all crypto and authentication, so it is not secure.
    /// Use it only for development and testing purposes!
    ///
    /// WARNING: **DO NOT USE DEVMODE IN PRODUCTION! IT FULLY DISABLES ENCRYPTION!!!**
    pub fn api_devmode(&self) -> bool {
        self.dev_mode.unwrap_or(false)
    }

    /// Return fileserver addr
    pub fn fileserver_bind_addr(&self) -> String {
        format!("{}:{}", self.fsr_ip.to_owned().unwrap_or(DEFAULT_ADDR.to_string()), self.fsr_port.unwrap_or(DEFAULT_FILESERVER_PORT))
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
        if alone { mr } else { self.fileserver_root().join(mr) }
    }

    /// Get default sysinspect root. For master it is always /etc/sysinspect
    pub fn root_dir(&self) -> PathBuf {
        PathBuf::from(DEFAULT_SYSINSPECT_ROOT.to_string())
    }

    /// Get minion keys store
    pub fn minion_keys_root(&self) -> PathBuf {
        self.root_dir().join(CFG_MINION_KEYS)
    }

    /// Get minion registry
    pub fn minion_registry_root(&self) -> PathBuf {
        self.root_dir().join(CFG_MINION_REGISTRY)
    }

    pub fn api_keys_root(&self) -> PathBuf {
        self.root_dir().join(CFG_API_KEYS)
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

    /// Return the path of the telemetry location
    pub fn telemetry_location(&self) -> PathBuf {
        PathBuf::from(self.telemetry_location.clone().unwrap_or(DEFAULT_MASTER_TELEMETRY_DB.to_string()))
    }

    /// Return the path of the telemetry communication socket location
    pub fn telemetry_socket(&self) -> PathBuf {
        PathBuf::from(self.telemetry_socket.clone().unwrap_or(DEFAULT_MASTER_TELEMETRY_SCK.to_string()))
    }

    /// Return the path of the telemetry communication socket location
    pub fn get_mod_repo_root(&self) -> PathBuf {
        self.fileserver_root().join(CFG_MODREPO_ROOT)
    }

    /// Returns true if telemetry is enabled
    pub fn telemetry_enabled(&self) -> bool {
        self.telemetry.is_some() && self.telemetry.as_ref().unwrap().collector.is_some()
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
struct SysInspectConfigs {
    #[serde(skip_serializing_if = "Option::is_none")]
    minion: Option<MinionConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    master: Option<MasterConfig>,
}

/// SysInspect configuration serialiser
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct SysInspectConfig {
    config: SysInspectConfigs,
}

impl SysInspectConfig {
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).unwrap_or_default()
    }

    pub fn to_value(&self) -> Value {
        serde_yaml::to_value(self).unwrap_or_default()
    }

    /// Set minion config
    pub fn set_minion_config(mut self, cfg: MinionConfig) -> Self {
        self.config.minion = Some(cfg);
        self
    }

    /// Set master config
    pub fn set_master_config(mut self, cfg: MasterConfig) -> Self {
        self.config.master = Some(cfg);
        self
    }
}
