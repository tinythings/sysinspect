/*
Inheritance patcher.

The weird/odd inheritance works the following way:

1. All replacement keys are extracted frin an inherited merged value
2. Keys are then applied to the base value accordingly
 */

use crate::util::dataconv;
use serde_yaml::Value;
use std::collections::HashMap;

static MOD_ADD: &str = "(+)";
static MOD_REMOVE: &str = "(-)";

fn _modpth(v: &Value, mut cp: Vec<String>, mut p: HashMap<Vec<String>, Value>) -> (&Value, HashMap<Vec<String>, Value>) {
    let mut cv: Option<Value> = None;

    if let Value::Mapping(map) = v {
        for (k, v) in map {
            let k = dataconv::as_str(Some(k).cloned());
            cp.push(k.to_owned());
            cv = Some(v.clone());
            if k.starts_with(MOD_REMOVE) || k.starts_with(MOD_ADD) {
                continue;
            }

            if let Value::Mapping(_) = v {
                (_, p) = _modpth(v, cp.clone(), p.clone());
            }
        }
    }

    // Add only modifier paths
    for e in &cp {
        if e.starts_with(MOD_ADD) || e.starts_with(MOD_REMOVE) {
            p.insert(cp, cv.unwrap());
            break;
        }
    }

    (v, p)
}

/// Get modification paths, those have prefixes to add `(+)` or remove `(-)`.
pub fn get_modifiers(v: &Value) -> HashMap<Vec<String>, Value> {
    _modpth(v, vec![], HashMap::default()).1
}
