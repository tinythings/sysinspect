use std::{fs, path::PathBuf};

use indexmap::IndexMap;
use serde_json::{json, Value};

/// SystemTraits contains a key/value of a system properties.
#[derive(Debug, Clone)]
pub struct SystemTraits {
    data: IndexMap<String, Value>,
}

impl SystemTraits {
    pub fn new() -> SystemTraits {
        log::debug!("Initialising system traits");
        let mut traits = SystemTraits::default();
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

    /// Read standard system traits
    fn get_system(&mut self) {
        log::debug!("Reading system traits data");
        let system = sysinfo::System::new_all();

        // Common
        if let Some(v) = sysinfo::System::host_name() {
            self.put("system.hostname".to_string(), json!(v));
        }

        if let Some(v) = sysinfo::System::kernel_version() {
            self.put("system.kernel".to_string(), json!(v));
        }

        if let Some(v) = sysinfo::System::os_version() {
            self.put("system.os.version".to_string(), json!(v));
        }

        if let Some(v) = sysinfo::System::name() {
            self.put("system.os.name".to_string(), json!(v));
        }

        self.put("system.os.distribution".to_string(), json!(sysinfo::System::distribution_id()));

        // Machine Id (not always there)
        let mip = PathBuf::from("/etc/machine-id");
        let mut mid = String::default();
        if mip.exists() {
            if let Ok(id) = fs::read_to_string(mip) {
                mid = id.trim().to_string();
            }
        }
        self.put("system.id".to_string(), json!(mid));

        // Memory
        self.put("system.mem.total".to_string(), json!(system.total_memory()));
        self.put("system.swap.total".to_string(), json!(system.total_swap()));

        // Load CPU data
        self.put("system.cpu.total".to_string(), json!(system.cpus().len()));
        self.put("system.cpu.brand".to_string(), json!(system.cpus()[0].brand()));
        self.put("system.cpu.frequency".to_string(), json!(system.cpus()[0].frequency()));
        self.put("system.cpu.vendor".to_string(), json!(system.cpus()[0].vendor_id()));
        if let Some(pcrc) = system.physical_core_count() {
            self.put("system.cpu.cores".to_string(), json!(pcrc));
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
    fn get_defined(&self) {
        log::debug!("Reading custon static traits data")
    }
}

impl Default for SystemTraits {
    fn default() -> Self {
        Self { data: Default::default() }
    }
}
