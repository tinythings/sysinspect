/*
Functions
 */

use crate::{intp::functions::ModArgFunction, SysinspectError};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::hash::Hash;

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
        self.as_array().map(|v| &v[..])
    }

    fn is_object(&self) -> bool {
        self.is_object()
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
        if let YamlValue::Sequence(seq) = self {
            Some(seq)
        } else {
            None
        }
    }

    fn is_object(&self) -> bool {
        matches!(self, YamlValue::Mapping(_))
    }

    fn get_by_key(&self, key: &Self::Key) -> Option<&Self> {
        if let YamlValue::Mapping(map) = self {
            let key_value = YamlValue::String(key.clone());
            map.get(&key_value)
        } else {
            None
        }
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
        let n = ns[0];
        let key = n.to_string();
        if let Some(v) = data.get_by_key(&key) {
            get_ns_val(v, &ns[1..])
        } else {
            None
        }
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
pub fn is_function(arg: &str) -> Result<Option<ModArgFunction>, SysinspectError> {
    if !arg.contains("(") || !arg.ends_with(")") {
        return Ok(None);
    }

    let f = ModArgFunction::new(
        arg.split('(').nth(1).and_then(|s| s.split(')').next()).unwrap_or_default().to_string(),
        arg.split("(").next().unwrap_or_default().to_string(),
    )?;

    Ok(Some(f))
}
