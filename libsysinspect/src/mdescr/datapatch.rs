/*
Inheritance patcher.
The goal is to take an inherited Model and modify it accordingly.
*/

use crate::util::dataconv;
use indexmap::IndexMap;
use serde_yaml::{Mapping, Value};

static MOD_REMOVE: &str = "(-)";

/// Get modification paths
fn modpth(v: &Value, path: &mut Vec<String>, result: &mut IndexMap<Vec<String>, Value>) {
    match v {
        Value::Mapping(map) => {
            for (k, v) in map {
                path.push(dataconv::as_str(Some(k).cloned()));
                modpth(v, path, result);
                path.pop();
            }
        }
        _ => {
            result.insert(path.clone(), v.clone());
        }
    }
}

/// Apply modification paths to the target structure
fn modbase(base: &mut Value, mods: IndexMap<Vec<String>, Value>) {
    for (pth, v) in mods {
        let mut cv = &mut *base;

        for (i, k) in pth.iter().enumerate() {
            let (clr_k, rm) = if k.starts_with(MOD_REMOVE) {
                (k.trim_start_matches(MOD_REMOVE).trim().to_string(), true)
            } else {
                (k.clone(), false)
            };

            let next = {
                if let Value::Mapping(m) = cv {
                    if rm {
                        m.remove(Value::String(clr_k.clone()));
                        None
                    } else if i == pth.len() - 1 {
                        m.insert(Value::String(clr_k), v.clone());
                        None
                    } else {
                        Some(m.entry(Value::String(clr_k)).or_insert_with(|| Value::Mapping(Mapping::new())))
                    }
                } else {
                    None
                }
            };
            if let Some(next) = next {
                cv = next;
            } else {
                break;
            }
        }
    }
}

/// Inherit from a model description
pub fn inherit(base: &mut Value, ovl: &Value) {
    let mut mpth: IndexMap<Vec<String>, Value> = IndexMap::new();
    modpth(ovl, &mut vec![], &mut mpth);
    modbase(base, mpth);
}
