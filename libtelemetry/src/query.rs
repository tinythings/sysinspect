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
        log::info!("Loading data for path for key {:#?}: {:#?}", k, jpath);
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
