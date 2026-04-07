use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MinionRecord {
    id: String,
    traits: HashMap<String, Value>,
    #[serde(default)]
    static_keys: BTreeSet<String>,
    #[serde(default)]
    fn_keys: BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MinionCmdbStartup {
    user: String,
    host: String,
    root: String,
    bin: String,
    path: String,
    backend: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct MinionCmdbRecord {
    mid: String,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    fqdn: Option<String>,
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    root: Option<String>,
    #[serde(default)]
    bin: Option<String>,
    #[serde(default)]
    config: Option<String>,
    #[serde(default)]
    backend: Option<String>,
    updated_at: DateTime<Utc>,
}

impl MinionCmdbStartup {
    pub fn new(user: String, host: String, root: String, bin: String, path: String, backend: String) -> Self {
        Self { user, host, root, bin, path, backend }
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn bin(&self) -> &str {
        &self.bin
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn backend(&self) -> &str {
        &self.backend
    }
}

impl MinionCmdbRecord {
    pub fn new(mid: String) -> Self {
        Self {
            mid,
            user: None,
            host: None,
            hostname: None,
            fqdn: None,
            ip: None,
            root: None,
            bin: None,
            config: None,
            backend: None,
            updated_at: Utc::now(),
        }
    }

    pub fn apply_startup(&mut self, startup: &MinionCmdbStartup) {
        self.user = Some(startup.user().to_string());
        self.host = Some(startup.host().to_string());
        self.root = Some(startup.root().to_string());
        self.bin = Some(startup.bin().to_string());
        self.config = Some(startup.path().to_string());
        self.backend = Some(startup.backend().to_string());
        self.updated_at = Utc::now();
    }

    pub fn apply_observed_traits(&mut self, traits: &HashMap<String, Value>) {
        for (key, slot) in [("system.hostname", &mut self.hostname), ("system.hostname.fqdn", &mut self.fqdn), ("system.hostname.ip", &mut self.ip)] {
            if let Some(value) = traits.get(key).and_then(|value| value.as_str()) {
                *slot = Some(value.to_string());
            }
        }

        self.updated_at = Utc::now();
    }

    pub fn is_stale(&self, max_age: std::time::Duration) -> bool {
        chrono::Duration::from_std(max_age).map(|max_age| Utc::now() - self.updated_at >= max_age).unwrap_or(false)
    }

    pub fn mid(&self) -> &str {
        &self.mid
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    pub fn hostname(&self) -> Option<&str> {
        self.hostname.as_deref()
    }

    pub fn fqdn(&self) -> Option<&str> {
        self.fqdn.as_deref()
    }

    pub fn ip(&self) -> Option<&str> {
        self.ip.as_deref()
    }

    pub fn root(&self) -> Option<&str> {
        self.root.as_deref()
    }

    pub fn bin(&self) -> Option<&str> {
        self.bin.as_deref()
    }

    pub fn config(&self) -> Option<&str> {
        self.config.as_deref()
    }

    pub fn backend(&self) -> Option<&str> {
        self.backend.as_deref()
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    #[cfg(test)]
    pub fn set_updated_at(&mut self, updated_at: DateTime<Utc>) {
        self.updated_at = updated_at;
    }
}

impl MinionRecord {
    pub fn new(id: String, traits: HashMap<String, Value>, static_keys: BTreeSet<String>, fn_keys: BTreeSet<String>) -> Self {
        MinionRecord { id, traits, static_keys, fn_keys }
    }

    /// Check if the record matches the value
    pub fn matches(&self, attr: &str, v: Value) -> bool {
        self.traits.get(attr).map(|f| f.eq(&v)).unwrap_or(false)
    }

    // Get minion id
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn matches_selectors(&self, set: Vec<String>) -> bool {
        if set.is_empty() || set.contains(&"*".to_string()) {
            return true;
        }

        let mut matched = 0;
        for selector in &set {
            if !selector.contains(":") {
                log::warn!("Invalid selector format: {selector}");
                continue;
            }

            let parts: Vec<&str> = selector.split(':').collect(); // attr:value
            if parts.len() != 2 {
                log::warn!("Invalid selector format: {selector}");
                continue;
            }

            if libtelemetry::expr::expr(parts[1], self.traits.get(parts[0]).cloned().unwrap_or_default()) {
                matched += 1;
            }
        }
        matched == set.len()
    }

    pub fn get_traits(&self) -> &HashMap<String, Value> {
        &self.traits
    }

    pub fn is_function_trait(&self, key: &str) -> bool {
        self.fn_keys.contains(key)
    }

    pub fn is_yaml_trait(&self, key: &str) -> bool {
        self.static_keys.contains(key)
    }
}
