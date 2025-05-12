use std::collections::HashMap;

use indexmap::IndexMap;
use jsonpath_rust::JsonPath;
use libsysinspect::SysinspectError;
use serde_json::Value;

/// Select data using JSONPath RFC9535
/// https://www.rfc-editor.org/rfc/rfc9535.html
pub fn select(jpath: &str, data: &Value) -> Result<Vec<Value>, SysinspectError> {
    let res = match data.query(jpath) {
        Ok(z) => z,
        Err(e) => {
            return Err(SysinspectError::JsonPathError(e.to_string()));
        }
    };

    Ok(res.into_iter().map(|v| v.to_owned()).collect())
}

/// Load data using JSONPath RFC9535
pub fn load_data(paths: IndexMap<String, String>, data: Value) -> Result<IndexMap<String, Value>, SysinspectError> {
    let mut out = IndexMap::new();
    for (k, jpath) in paths {
        let k = k.trim().to_string();
        let jpath = jpath.trim().to_string();
        let res = select(&jpath, &data)?;
        if res.is_empty() {
            return Err(SysinspectError::JsonPathError(format!("No data found for path: {}", k)));
        } else if res.len() > 1 {
            return Err(SysinspectError::JsonPathError(format!("Multiple data found for path: {}", k)));
        } else {
            out.insert(k, res[0].clone());
        }
    }

    Ok(out)
}

/// Cast data to the specified type
pub fn cast_data(data: &mut IndexMap<String, Value>, typemap: &IndexMap<String, String>) {
    for (key, val) in data.iter_mut() {
        if let Some(t) = typemap.get(key) {
            let t = t.trim().to_string();
            let v = match t.as_str() {
                "string" => Value::String(val.to_string()),
                "int" => Value::from(val.as_i64().unwrap_or_default()),
                "float" => Value::from(val.as_f64().unwrap_or_default()),
                _ => continue,
            };
            *val = v;
        }
    }
}

/// Interpolate data to format a string
pub fn interpolate_data(tpl: &str, data: &IndexMap<String, Value>) -> Result<String, Box<dyn std::error::Error>> {
    let mut vars = HashMap::new();
    for (k, v) in data.iter() {
        let s = match v {
            Value::String(s) => s.clone(),
            _ => v.to_string(),
        };
        vars.insert(k.clone(), s);
    }

    Ok(strfmt::strfmt(tpl, &vars)?)
}
