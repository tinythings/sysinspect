use indexmap::IndexMap;
use serde_json::Value;

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
    fn get_system(&self) {
        log::debug!("Reading system traits data");
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
