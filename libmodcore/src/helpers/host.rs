use serde_json::{Map, Value};

/// Shared runtime helper view over the descriptive `host` payload.
///
/// This stays intentionally small: it only reads existing request data and
/// does not invent side effects or hidden semantics.
pub struct RuntimeHost<'a> {
    host: &'a Value,
}

impl<'a> RuntimeHost<'a> {
    /// Build a helper view for a runtime `host` payload object.
    pub fn new(host: &'a Value) -> Self {
        Self { host }
    }

    /// Return a trait value by key from `host.traits`.
    pub fn trait_value(&self, name: &str) -> Option<Value> {
        self.traits().get(name).cloned()
    }

    /// Return whether a trait key exists in `host.traits`.
    pub fn has_trait(&self, name: &str) -> bool {
        self.traits().contains_key(name)
    }

    /// Return the `host.paths` object, or `{}` when absent.
    pub fn paths(&self) -> Value {
        Value::Object(self.paths_map().clone())
    }

    /// Return a path value by key from `host.paths`.
    pub fn path_value(&self, name: &str) -> Option<Value> {
        self.paths_map().get(name).cloned()
    }

    fn traits(&self) -> &Map<String, Value> {
        self.host.get("traits").and_then(Value::as_object).unwrap_or_else(|| empty_map())
    }

    fn paths_map(&self) -> &Map<String, Value> {
        self.host.get("paths").and_then(Value::as_object).unwrap_or_else(|| empty_map())
    }
}

fn empty_map() -> &'static Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(Map::new)
}
