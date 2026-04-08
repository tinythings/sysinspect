use crate::{
    cfg::mmconf::MinionConfig,
    traits::{
        HW_CPU_BRAND, HW_CPU_CORES, HW_CPU_FREQ, HW_CPU_TOTAL, HW_CPU_VENDOR, HW_MEM, HW_SWAP, MASTER_TRAITS_FILE, SYS_ARCH, SYS_ID,
        SYS_NET_HOSTNAME, SYS_NET_HOSTNAME_FQDN, SYS_NET_HOSTNAME_IP, SYS_OS_DISTRO, SYS_OS_KERNEL, SYS_OS_NAME, SYS_OS_VERSION, TraitSource,
        TraitsTransportPayload,
    },
    util::sys::to_fqdn_ip,
};
use indexmap::{IndexMap, IndexSet};
use libcommon::SysinspectError;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self},
    os::unix::fs::PermissionsExt,
    process::Command,
};

/// SystemTraits contains a key/value of a system properties.
#[derive(Debug, Clone, Default)]
pub struct SystemTraits {
    data: IndexMap<String, Value>,
    yaml_keys: IndexSet<String>,
    function_keys: IndexSet<String>,
    cfg: MinionConfig,
    checksum: String,
    quiet: bool,
}

impl SystemTraits {
    pub fn new(cfg: MinionConfig, quiet: bool) -> SystemTraits {
        if !quiet {
            log::debug!("Initialising system traits");
        }

        let mut traits = SystemTraits { cfg, quiet, ..Default::default() };
        traits.get_system();
        traits.get_network();
        if let Err(err) = traits.get_defined() {
            log::error!("Unable to load custom traits: {err}");
        }

        if let Err(err) = traits.get_functions() {
            log::error!("Unable to load trait functions: {err}");
        }

        traits
    }

    /// New SystemTraits from predefined map (IndexMap or HashMap etc)
    pub fn from_map<I>(predefined: I) -> SystemTraits
    where
        I: IntoIterator<Item = (String, Value)>,
    {
        let mut traits = SystemTraits { cfg: MinionConfig::default(), quiet: true, ..Default::default() };

        for (k, v) in predefined {
            traits.put(k, v);
        }

        traits
    }

    /// Put a JSON value into traits structure
    pub fn put(&mut self, path: String, data: Value) {
        self.yaml_keys.shift_remove(&path);
        self.function_keys.shift_remove(&path);
        self.data.insert(path, data);
    }

    /// Put a JSON value into traits structure and mark it as YAML drop-in generated.
    pub fn put_yaml(&mut self, path: String, data: Value) {
        self.function_keys.shift_remove(&path);
        self.yaml_keys.insert(path.clone());
        self.data.insert(path, data);
    }

    /// Put a JSON value into traits structure and mark it as function-generated.
    pub fn put_function(&mut self, path: String, data: Value) {
        self.yaml_keys.shift_remove(&path);
        self.function_keys.insert(path.clone());
        self.data.insert(path, data);
    }

    /// Get a trait value in JSON
    pub fn get(&self, path: &str) -> Option<Value> {
        self.data.get(path).cloned()
    }

    /// Check if trait is present
    pub fn has(&self, path: &str) -> bool {
        self.get(path).is_some()
    }

    /// Check if trait matches the requested value.
    pub fn matches(&self, path: &str, v: Value) -> bool {
        if let Some(t) = self.get(path) {
            return t.eq(&v);
        }

        false
    }

    /// Return known trait items
    pub fn trait_keys(&self) -> Vec<String> {
        let mut items = self.data.keys().map(|s| s.to_string()).collect::<Vec<String>>();
        items.sort();

        items
    }

    /// Return a cloned map of all known trait items.
    pub fn to_map(&self) -> IndexMap<String, Value> {
        self.data.clone()
    }

    /// Return the sorted list of keys that originated from trait functions.
    pub fn function_keys(&self) -> Vec<String> {
        self.function_keys.iter().cloned().collect()
    }

    /// Return the sorted list of keys that originated from YAML drop-in files.
    pub fn yaml_keys(&self) -> Vec<String> {
        self.yaml_keys.iter().cloned().collect()
    }

    /// Return checksum of traits.
    /// This is done by calculating checksum of the *keys*, as values can change on every restart,
    /// e.g. IPv6 data, which is usually irrelevant or any other things that *meant* to change.
    pub fn checksum(&mut self) -> String {
        if !self.checksum.is_empty() {
            return self.checksum.to_owned();
        }

        let mut keys = self.data.keys().map(|s| s.to_string()).collect::<Vec<String>>();
        keys.sort();

        self.checksum = format!("{:x}", Sha256::digest(keys.join("|").as_bytes()));
        self.checksum.to_owned()
    }

    /// Proxypass the error logging
    fn proxy_log_error<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(err) => {
                log::error!("{context}: {err}");
                None
            }
        }
    }

    /// Read standard system traits
    fn get_system(&mut self) {
        if !self.quiet {
            log::info!("Loading system traits data");
        }

        let system = sysinfo::System::new_all();

        // Common
        if let Some(v) = sysinfo::System::host_name() {
            self.put(SYS_NET_HOSTNAME.to_string(), json!(v));
        }

        let host_net = to_fqdn_ip(&sysinfo::System::host_name().unwrap_or_default());
        if let Some((fqdn, _)) = &host_net {
            self.put(SYS_NET_HOSTNAME_FQDN.to_string(), json!(fqdn));
        }
        if let Some(ipaddr) = host_net.map(|(_, ip)| ip).or_else(crate::util::sys::primary_ip) {
            self.put(SYS_NET_HOSTNAME_IP.to_string(), json!(ipaddr.to_string()));
        }

        if let Some(v) = sysinfo::System::kernel_version() {
            self.put(SYS_OS_KERNEL.to_string(), json!(v));
        }

        if let Some(v) = sysinfo::System::os_version() {
            self.put(SYS_OS_VERSION.to_string(), json!(v));
        }

        self.put(SYS_OS_NAME.to_string(), json!(super::os_display_name(super::current_os_type())));
        self.put(SYS_OS_DISTRO.to_string(), json!(super::current_os_type()));
        self.put(
            SYS_ARCH.to_string(),
            json!(match std::env::consts::ARCH {
                "aarch64" => "arm64",
                "arm" | "armv7" | "armv7l" => "arm",
                "riscv64" => "riscv",
                arch => arch,
            }),
        );

        // Machine Id (not always there)
        let mut mid = String::default();
        if self.cfg.machine_id_path().exists()
            && let Ok(id) = fs::read_to_string(self.cfg.machine_id_path())
        {
            mid = id.trim().to_string();
        }
        self.put(SYS_ID.to_string(), json!(mid));

        // Memory
        self.put(HW_MEM.to_string(), json!(system.total_memory()));
        self.put(HW_SWAP.to_string(), json!(system.total_swap()));

        // Load CPU data
        self.put(HW_CPU_TOTAL.to_string(), json!(system.cpus().len()));
        self.put(HW_CPU_BRAND.to_string(), json!(system.cpus()[0].brand()));
        self.put(HW_CPU_FREQ.to_string(), json!(system.cpus()[0].frequency()));
        self.put(HW_CPU_VENDOR.to_string(), json!(system.cpus()[0].vendor_id()));
        self.put(HW_CPU_CORES.to_string(), json!(system.cpus().len()));
    }

    /// Load network data
    fn get_network(&mut self) {
        if !self.quiet {
            log::info!("Loading network traits data");
        }
        let net = sysinfo::Networks::new_with_refreshed_list();
        for (ifs, data) in net.iter() {
            self.put(format!("system.net.{ifs}.mac"), json!(data.mac_address().to_string()));
            for ipn in data.ip_networks() {
                let tp = if ipn.addr.is_ipv4() { "4" } else { "6" };
                self.put(format!("system.net.{ifs}.ipv{tp}"), json!(ipn.addr.to_string()));
            }
        }
    }

    /// Read defined/configured static traits
    fn get_defined(&mut self) -> Result<(), SysinspectError> {
        if !self.quiet {
            log::info!("Loading defined/custom traits");
        }

        let mut files = fs::read_dir(self.cfg.traits_dir())?
            .flatten()
            .filter(|f| f.file_name().to_str().unwrap_or_default().ends_with(".cfg"))
            .collect::<Vec<_>>();
        files.sort_by_key(|f| {
            let name = f.file_name().to_str().unwrap_or_default().to_string();
            (name == MASTER_TRAITS_FILE, name)
        });

        for f in files {
            let fname = f.file_name();
            let fname = fname.to_str().unwrap_or_default();
            let content = Self::proxy_log_error(fs::read_to_string(f.path()), format!("Unable to read custom trait file at {fname}").as_str())
                .unwrap_or_default();

            if content.is_empty() {
                continue;
            }

            let content: Option<serde_yaml::Value> = Self::proxy_log_error(serde_yaml::from_str(&content), "Custom trait file has broken YAML");

            let content: Option<serde_json::Value> =
                content.as_ref().and_then(|v| Self::proxy_log_error(serde_json::to_value(v), "Unable to convert existing YAML to JSON format"));

            if fname == MASTER_TRAITS_FILE && content.as_ref().is_none_or(serde_json::Value::is_null) {
                continue;
            }

            if content.is_none() {
                log::error!("Unable to load custom traits from {}", f.file_name().to_str().unwrap_or_default());
                continue;
            }

            let content = content.as_ref().and_then(|v| {
                Self::proxy_log_error(serde_json::from_value::<IndexMap<String, serde_json::Value>>(v.clone()), "Unable to parse JSON")
            });

            if let Some(content) = content {
                for (k, v) in content {
                    self.put_yaml(k, json!(v));
                }
            } else {
                log::error!("Custom traits data is empty or in a wrong format");
            }
        }
        Ok(())
    }

    /// Load custom functions
    fn get_functions(&mut self) -> Result<(), SysinspectError> {
        if !self.quiet {
            log::info!("Loading trait functions");
        }
        for f in fs::read_dir(self.cfg.functions_dir())?.flatten() {
            let fname = f.path();
            let fname = fname.file_name().unwrap_or_default().to_str().unwrap_or_default();

            if !self.quiet {
                log::info!("Calling function {fname}");
            }

            let is_exec = match fs::metadata(f.path()) {
                Ok(m) => {
                    #[cfg(unix)]
                    {
                        m.permissions().mode() & 0o111 != 0
                    }
                }
                Err(_) => false,
            };

            if is_exec {
                let out = Command::new(f.path()).output()?;
                if out.status.success() {
                    let data =
                        Self::proxy_log_error(String::from_utf8(out.stdout), format!("Unable to load content from the function {fname}").as_str());
                    if data.is_none() {
                        log::error!("Function {fname} returned no content");
                        continue;
                    }
                    let data = data.unwrap_or_default();
                    let data = Self::proxy_log_error(
                        serde_json::from_str::<IndexMap<String, serde_json::Value>>(&data),
                        format!("Unable to parse JSON output from trait function at {fname}").as_str(),
                    );
                    if let Some(data) = data {
                        for (k, v) in data {
                            self.put_function(k, json!(v));
                        }
                    } else {
                        log::error!("Custom traits data is empty or in a wrong format");
                    }
                } else {
                    log::error!("Error running {fname}");
                }
            } else {
                log::warn!("Function {fname} is not an executable, skipping");
            }
        }
        Ok(())
    }

    /// Convert the data to the JSON body
    pub fn to_json_string(&self) -> Result<String, SysinspectError> {
        Ok(serde_json::to_string(&json!(self.data))?)
    }

    /// Convert the data to the JSON value
    pub fn to_json_value(&self) -> Result<Value, SysinspectError> {
        Ok(json!(self.data))
    }

    /// Convert traits into the structured transport payload used for sync with the master.
    pub fn to_transport_value(&self) -> Result<Value, SysinspectError> {
        Ok(serde_json::to_value(TraitsTransportPayload { traits: self.data.clone(), static_keys: self.yaml_keys(), fn_keys: self.function_keys() })?)
    }

    /// Return the origin category for one trait key.
    pub fn trait_source(&self, key: &str) -> TraitSource {
        if self.function_keys.contains(key) {
            TraitSource::Function
        } else if self.yaml_keys.contains(key) {
            TraitSource::Static
        } else {
            TraitSource::Preset
        }
    }
}
