use libcommon::SysinspectError;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::hash::Hash;

pub enum StaticNamespace {
    SECTION = 0,
    ENTITY = 1,
    REGION = 2,
    STATE = 3,
    LABEL = 4,
}

pub enum ClaimNamespace {
    LABEL = 0,
}

pub trait ExtValue: Clone {
    type Key: Clone + Eq + Hash;

    fn is_array(&self) -> bool;
    fn as_array(&self) -> Option<&[Self]>;
    fn is_object(&self) -> bool;
    fn get_by_key(&self, key: &Self::Key) -> Option<&Self>;
}

impl ExtValue for JsonValue {
    type Key = String;

    fn is_array(&self) -> bool {
        self.is_array()
    }

    fn as_array(&self) -> Option<&[Self]> {
        if let JsonValue::Array(seq) = self { Some(seq) } else { None }
    }

    fn is_object(&self) -> bool {
        matches!(self, JsonValue::Object(_))
    }

    fn get_by_key(&self, key: &Self::Key) -> Option<&Self> {
        self.get(key)
    }
}

impl ExtValue for YamlValue {
    type Key = String;

    fn is_array(&self) -> bool {
        matches!(self, YamlValue::Sequence(_))
    }

    fn as_array(&self) -> Option<&[Self]> {
        if let YamlValue::Sequence(seq) = self { Some(seq) } else { None }
    }

    fn is_object(&self) -> bool {
        matches!(self, YamlValue::Mapping(_))
    }

    fn get_by_key(&self, key: &Self::Key) -> Option<&Self> {
        if let YamlValue::Mapping(map) = self { map.get(YamlValue::String(key.clone())) } else { None }
    }
}

fn get_ns_val<T>(data: &T, ns: &[&str]) -> Option<T>
where
    T: ExtValue<Key = String>,
{
    if ns.is_empty() {
        return Some(data.clone());
    }

    if data.is_array() {
        if let Some(arr) = data.as_array() {
            for item in arr {
                if let Some(v) = get_ns_val(item, ns) {
                    return Some(v);
                }
            }
        }
        None
    } else if data.is_object() {
        if let Some(v) = data.get_by_key(&ns[0].to_string()) { get_ns_val(v, &ns[1..]) } else { None }
    } else {
        None
    }
}

/// Get data structure from the Value by namespace.
pub fn get_by_namespace<T>(data: Option<T>, namespace: &str) -> Option<T>
where
    T: ExtValue<Key = String>,
{
    if let Some(ref data) = data {
        let ns: Vec<&str> = namespace.split('.').collect();
        get_ns_val(data, &ns)
    } else {
        None
    }
}

/// Detect if an argument is a function
pub fn is_function(arg: &YamlValue) -> Result<Option<ModArgFunction>, SysinspectError> {
    let arg = match arg.as_str() {
        Some(s) => s,
        None => return Ok(None),
    };

    if !arg.contains("(") || !arg.ends_with(")") {
        return Ok(None);
    }

    let f = ModArgFunction::new(
        arg.split('(').nth(1).and_then(|s| s.split(')').next()).unwrap_or_default().to_string(),
        arg.split("(").next().unwrap_or_default().to_string(),
    )?;

    Ok(Some(f))
}

#[derive(Debug, Default)]
pub struct ModArgFunction {
    namespace: Vec<String>,
    fid: String,
}

impl ModArgFunction {
    pub fn new(ns: String, fid: String) -> Result<Self, SysinspectError> {
        let namespace = ns.split('.').map(|s| s.to_string()).collect::<Vec<String>>();

        if namespace.len() < 2 && !fid.eq("context") {
            return Err(SysinspectError::ModelDSLError(format!("Function {fid} does not have at least two fold namespace: {ns}")));
        }

        Ok(ModArgFunction { namespace, fid })
    }

    /// Get function namespace
    pub fn namespace(&self) -> String {
        self.namespace.join(".").to_string()
    }

    pub fn ns(&self) -> Vec<String> {
        self.namespace.to_owned()
    }

    /// Get function Id
    pub fn fid(&self) -> &str {
        &self.fid
    }
}

/// Deep-merge two serde_yaml or serde_json `Value` trees.
///
/// Scalars are replaced, mappings are recursively merged,
/// and sequences are appended.
pub(crate) fn deep_merge(base: &mut serde_yaml::Value, overlay: &serde_yaml::Value) {
    use serde_yaml::Value;

    match (base, overlay) {
        (Value::Mapping(b), Value::Mapping(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(existing) => deep_merge(existing, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (Value::Sequence(b), Value::Sequence(o)) => {
            b.extend(o.iter().cloned());
        }
        (b, o) => {
            *b = o.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_replace() {
        let mut base = serde_yaml::from_str::<serde_yaml::Value>("port: 4200").unwrap();
        let overlay = serde_yaml::from_str::<serde_yaml::Value>("port: 9999").unwrap();
        deep_merge(&mut base, &overlay);
        assert_eq!(base["port"].as_u64().unwrap(), 9999);
    }

    #[test]
    fn map_deep_merge() {
        let mut base = serde_yaml::from_str::<serde_yaml::Value>("a: {x: 1, y: 2}").unwrap();
        let overlay = serde_yaml::from_str::<serde_yaml::Value>("a: {y: 99, z: 3}").unwrap();
        deep_merge(&mut base, &overlay);
        assert_eq!(base["a"]["x"].as_u64().unwrap(), 1);
        assert_eq!(base["a"]["y"].as_u64().unwrap(), 99);
        assert_eq!(base["a"]["z"].as_u64().unwrap(), 3);
    }

    #[test]
    fn sequence_append() {
        let mut base = serde_yaml::from_str::<serde_yaml::Value>("items: [a, b]").unwrap();
        let overlay = serde_yaml::from_str::<serde_yaml::Value>("items: [c, d]").unwrap();
        deep_merge(&mut base, &overlay);
        let seq = base["items"].as_sequence().unwrap();
        assert_eq!(seq.len(), 4);
        assert_eq!(seq[0].as_str().unwrap(), "a");
        assert_eq!(seq[2].as_str().unwrap(), "c");
    }

    #[test]
    fn new_key_added() {
        let mut base = serde_yaml::from_str::<serde_yaml::Value>("a: 1").unwrap();
        let overlay = serde_yaml::from_str::<serde_yaml::Value>("b: 2").unwrap();
        deep_merge(&mut base, &overlay);
        assert_eq!(base["a"].as_u64().unwrap(), 1);
        assert_eq!(base["b"].as_u64().unwrap(), 2);
    }
}
