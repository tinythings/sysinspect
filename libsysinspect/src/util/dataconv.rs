/*
Convenience functions to save the boilerplate.
*/

use rustpython_vm::{AsObject, PyObjectRef, VirtualMachine};
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;

/// Extended value, composing JSON and YAML together
pub trait ExtValue {
    fn as_str_opt(&self) -> Option<String>;
    fn as_str_list_opt(&self) -> Option<Vec<String>>;
    fn as_i64_opt(&self) -> Option<i64>;
    fn as_bool_opt(&self) -> Option<bool>;
    fn to_string(&self) -> Option<String>;
    fn to_pyobjectref(&self, vm: &VirtualMachine) -> PyObjectRef;
}

impl ExtValue for YamlValue {
    fn as_str_opt(&self) -> Option<String> {
        if let YamlValue::String(v) = self {
            return Some(v.to_owned());
        }

        None
    }

    fn as_str_list_opt(&self) -> Option<Vec<String>> {
        if let YamlValue::Sequence(v) = self {
            let mut out: Vec<String> = Vec::default();
            for i in v {
                if let Some(i) = i.as_str() {
                    out.push(i.to_string());
                }
            }
            return if v.len() == out.len() { Some(out) } else { None };
        }

        None
    }

    fn as_i64_opt(&self) -> Option<i64> {
        if let YamlValue::Number(v) = self {
            return v.as_i64();
        }

        None
    }

    fn as_bool_opt(&self) -> Option<bool> {
        if let YamlValue::Bool(v) = self {
            return Some(v.to_owned());
        }

        None
    }

    fn to_string(&self) -> Option<String> {
        match self {
            YamlValue::Null => Some("".to_string()),
            YamlValue::Bool(_) => Some((if as_bool(Some(self).cloned()) { "true" } else { "false" }).to_string()),
            YamlValue::Number(v) => Some(v.to_string()),
            YamlValue::String(v) => Some(v.to_string()),
            YamlValue::Sequence(_) => Some(as_str_list(Some(self).cloned()).join(",")),
            _ => None,
        }
    }

    fn to_pyobjectref(&self, vm: &VirtualMachine) -> PyObjectRef {
        match self {
            YamlValue::Null => vm.ctx.none(),
            YamlValue::Bool(b) => vm.ctx.new_bool(*b).into(),
            YamlValue::Number(num) => {
                if let Some(i) = num.as_i64() {
                    vm.ctx.new_int(i).into()
                } else if let Some(f) = num.as_f64() {
                    vm.ctx.new_float(f).into()
                } else {
                    vm.ctx.none()
                }
            }
            YamlValue::String(s) => vm.new_pyobj(s),
            YamlValue::Sequence(vec) => {
                let py_list = vm.ctx.new_list(vec.iter().map(|item| item.to_pyobjectref(vm)).collect());
                py_list.into()
            }
            YamlValue::Mapping(obj) => {
                let py_dict = vm.ctx.new_dict();
                for (key, val) in obj {
                    let py_key = vm.ctx.new_str(key.as_str().unwrap_or_default());
                    let py_val = val.to_pyobjectref(vm);
                    py_dict.set_item(py_key.as_object(), py_val, vm).expect("Failed to set dict item");
                }
                py_dict.into()
            }
            _ => vm.ctx.none(),
        }
    }
}

impl ExtValue for JsonValue {
    fn as_str_opt(&self) -> Option<String> {
        if let JsonValue::String(v) = self {
            return Some(v.to_owned());
        }

        None
    }

    fn as_str_list_opt(&self) -> Option<Vec<String>> {
        if let JsonValue::Array(v) = self {
            let mut out: Vec<String> = Vec::default();
            for i in v {
                if let Some(i) = i.as_str() {
                    out.push(i.to_string());
                }
            }
            return if v.len() == out.len() { Some(out) } else { None };
        }

        None
    }

    fn as_i64_opt(&self) -> Option<i64> {
        if let JsonValue::Number(v) = self {
            return v.as_i64();
        }

        None
    }

    fn as_bool_opt(&self) -> Option<bool> {
        if let JsonValue::Bool(v) = self {
            return Some(v.to_owned());
        }

        None
    }

    fn to_string(&self) -> Option<String> {
        match self {
            JsonValue::Null => Some("".to_string()),
            JsonValue::Bool(_) => Some((if as_bool(Some(self).cloned()) { "true" } else { "false" }).to_string()),
            JsonValue::Number(v) => Some(v.to_string()),
            JsonValue::String(v) => Some(v.to_string()),
            JsonValue::Array(_) => Some(as_str_list(Some(self).cloned()).join(",")),
            _ => None,
        }
    }

    fn to_pyobjectref(&self, vm: &VirtualMachine) -> PyObjectRef {
        match self {
            JsonValue::Null => vm.ctx.none(),
            JsonValue::Bool(b) => vm.ctx.new_bool(*b).into(),
            JsonValue::Number(num) => {
                if let Some(i) = num.as_i64() {
                    vm.ctx.new_int(i).into()
                } else if let Some(f) = num.as_f64() {
                    vm.ctx.new_float(f).into()
                } else {
                    vm.ctx.none()
                }
            }
            JsonValue::String(s) => vm.new_pyobj(s),
            JsonValue::Array(vec) => {
                let py_list = vm.ctx.new_list(vec.iter().map(|item| item.to_pyobjectref(vm)).collect());
                py_list.into()
            }
            JsonValue::Object(obj) => {
                let py_dict = vm.ctx.new_dict();
                for (key, val) in obj {
                    let py_key = vm.ctx.new_str(key.as_str());
                    let py_val = val.to_pyobjectref(vm);
                    py_dict.set_item(py_key.as_object(), py_val, vm).expect("Failed to set dict item");
                }
                py_dict.into()
            }
        }
    }
}

/// Get value as an optional string. If the value is not a string,
/// then `None` is returned.
pub fn as_str_opt<V: ExtValue>(v: Option<V>) -> Option<String> {
    v.and_then(|v| v.as_str_opt())
}

/// Get value as a string. If the value is not a string,
/// then an empty string is returned.
pub fn as_str<V: ExtValue>(v: Option<V>) -> String {
    if let Some(v) = as_str_opt(v) {
        return v;
    }
    "".to_string()
}

/// Return an optional list of strings either from a list type or
/// comma-separated string. If the value is not a list, or a list
/// of different types, `None` is returned.
pub fn as_str_list_opt<V: ExtValue>(v: Option<V>) -> Option<Vec<String>> {
    if let Some(v) = v.as_ref().and_then(|v| v.as_str_list_opt()) {
        return Some(v.to_vec());
    } else if let Some(v) = v.as_ref().and_then(|v| v.as_str_opt()) {
        if !v.contains(',') {
            return None;
        }
        return Some(v.split(',').map(|s| s.trim().to_string()).collect());
    }

    None
}

/// Return a list of strings. If the value is not a list,
/// or a list of different types, an empty list is returned.
pub fn as_str_list<V: ExtValue>(v: Option<V>) -> Vec<String> {
    if let Some(v) = as_str_list_opt(v) {
        return v;
    }

    vec![]
}

/// Get an optional integer
pub fn as_int_opt<V: ExtValue>(v: Option<V>) -> Option<i64> {
    v.and_then(|v| v.as_i64_opt())
}

/// Get an integer, defaulted to `0`.
pub fn as_int<V: ExtValue>(v: Option<V>) -> i64 {
    if let Some(v) = as_int_opt(v) {
        return v;
    }

    0
}

/// Get an optional boolean
pub fn as_bool_opt<V: ExtValue>(v: Option<V>) -> Option<bool> {
    v.and_then(|v| v.as_bool_opt())
}

/// Get a boolean, defaulted to `false`.
pub fn as_bool<V: ExtValue>(v: Option<V>) -> bool {
    if let Some(v) = as_bool_opt(v) {
        return v;
    }

    false
}

/// Convert a value to a string
pub fn to_string<V: ExtValue>(v: Option<V>) -> Option<String> {
    v.and_then(|v| v.to_string())
}

/// Convert a value to a PyObjectRef
pub fn to_pyobjectref<V: ExtValue>(v: Option<V>, vm: &VirtualMachine) -> Option<PyObjectRef> {
    if let Some(v) = v {
        return Some(v.to_pyobjectref(vm));
    }

    None
}
