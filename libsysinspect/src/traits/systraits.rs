use crate::{
    cfg::mmconf::MinionConfig,
    traits::{
        HW_CPU_BRAND, HW_CPU_CORES, HW_CPU_FREQ, HW_CPU_TOTAL, HW_CPU_VENDOR, HW_MEM, HW_SWAP, SYS_ID, SYS_NET_HOSTNAME,
        SYS_OS_DISTRO, SYS_OS_KERNEL, SYS_OS_NAME, SYS_OS_VERSION,
    },
    SysinspectError,
};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::PathBuf};

/// SystemTraits contains a key/value of a system properties.
#[derive(Debug, Clone, Default)]
pub struct SystemTraits {
    data: IndexMap<String, Value>,
    cfg: MinionConfig,
}

impl SystemTraits {
    pub fn new(cfg: MinionConfig) -> SystemTraits {
        log::debug!("Initialising system traits");
        let mut traits = SystemTraits { cfg, ..Default::default() };
        traits.get_system();
        traits.get_network();
        traits.get_defined();

        traits
    }

    /// Put a JSON value into traits structure
    pub fn put(&mut self, path: String, data: Value) {
        self.data.insert(path, data);
    }

    /// Get a trait value in JSON
    pub fn get(&self, path: String) -> Option<Value> {
        self.data.get(&path).cloned()
    }

    /// Check if trait is present
    pub fn has(&self, path: String) -> bool {
        self.get(path).is_some()
    }

    /// Check if trait matches the requested value.
    pub fn matches(&self, path: String, v: Value) -> bool {
        if let Some(t) = self.get(path) {
            return t.eq(&v);
        }

        false
    }

    /// Return known trait items
    pub fn items(&self) -> Vec<String> {
        self.data.keys().map(|s| s.to_string()).collect::<Vec<String>>()
    }

    /// Proxypass the error logging
    fn proxy_log_error<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(err) => {
                log::error!("{}: {}", context, err);
                None
            }
        }
    }

    /// Read standard system traits
    fn get_system(&mut self) {
        log::debug!("Reading system traits data");
        let system = sysinfo::System::new_all();

        // Common
        if let Some(v) = sysinfo::System::host_name() {
            self.put(SYS_NET_HOSTNAME.to_string(), json!(v));
        }

        if let Some(v) = sysinfo::System::kernel_version() {
            self.put(SYS_OS_KERNEL.to_string(), json!(v));
        }

        if let Some(v) = sysinfo::System::os_version() {
            self.put(SYS_OS_VERSION.to_string(), json!(v));
        }

        if let Some(v) = sysinfo::System::name() {
            self.put(SYS_OS_NAME.to_string(), json!(v));
        }

        self.put(SYS_OS_DISTRO.to_string(), json!(sysinfo::System::distribution_id()));

        // Machine Id (not always there)
        let mip = PathBuf::from("/etc/machine-id");
        let mut mid = String::default();
        if mip.exists() {
            if let Ok(id) = fs::read_to_string(mip) {
                mid = id.trim().to_string();
            }
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
        if let Some(pcrc) = system.physical_core_count() {
            self.put(HW_CPU_CORES.to_string(), json!(pcrc));
        }
    }

    /// Load network data
    fn get_network(&mut self) {
        log::debug!("Reading network traits data");
        let net = sysinfo::Networks::new_with_refreshed_list();
        for (ifs, data) in net.iter() {
            self.put(format!("system.net.{}.mac", ifs), json!(data.mac_address().to_string()));
            for ipn in data.ip_networks() {
                let tp = if ipn.addr.is_ipv4() { "4" } else { "6" };
                self.put(format!("system.net.{}.ipv{}", ifs, tp), json!(ipn.addr.to_string()));
            }
        }
    }

    /// Read defined/configured static traits
    fn get_defined(&mut self) -> Result<(), SysinspectError> {
        log::debug!("Reading custon static traits data");

        for f in fs::read_dir(self.cfg.traits_dir())?.flatten() {
            let fname = f.file_name();
            let fname = fname.to_str().unwrap_or_default();
            if fname.ends_with(".cfg") {
                let content = Self::proxy_log_error(
                    fs::read_to_string(f.path()),
                    format!("Unable to read custom trait file at {}", fname).as_str(),
                )
                .unwrap_or_default();

                if content.is_empty() {
                    continue;
                }

                let content: Option<serde_yaml::Value> =
                    Self::proxy_log_error(serde_yaml::from_str(&content), "Custom trait file has broken YAML");

                let content: Option<serde_json::Value> = content.as_ref().and_then(|v| {
                    Self::proxy_log_error(serde_json::to_value(v), "Unable to convert existing YAML to JSON format")
                });

                if content.is_none() {
                    log::error!("Unable to load custom traits from {}", f.file_name().to_str().unwrap_or_default());
                    continue;
                }

                let content = content.as_ref().and_then(|v| {
                    Self::proxy_log_error(
                        serde_json::from_value::<HashMap<String, serde_json::Value>>(v.clone()),
                        "Unable to parse JSON",
                    )
                });

                if let Some(content) = content {
                    for (k, v) in content {
                        self.put(k, json!(v));
                    }
                } else {
                    log::error!("Custom traits data is empty or in a wrong format");
                }
            }
        }
        Ok(())
    }
}
