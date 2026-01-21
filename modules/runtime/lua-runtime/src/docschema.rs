use crate::lrt::LuaRuntimeError;
use serde_json::Value as JsonValue;

fn expect_string<'a>(v: &'a JsonValue, path: &str) -> Result<&'a str, LuaRuntimeError> {
    v.as_str().ok_or_else(|| mlua::Error::runtime(format!("{path} must be a string")).into())
}

fn expect_bool(v: &JsonValue, path: &str) -> Result<bool, LuaRuntimeError> {
    v.as_bool().ok_or_else(|| mlua::Error::runtime(format!("{path} must be a boolean")).into())
}

fn expect_array<'a>(v: &'a JsonValue, path: &str) -> Result<&'a Vec<JsonValue>, LuaRuntimeError> {
    v.as_array().ok_or_else(|| mlua::Error::runtime(format!("{path} must be an array")).into())
}

fn expect_object<'a>(v: &'a JsonValue, path: &str) -> Result<&'a serde_json::Map<String, JsonValue>, LuaRuntimeError> {
    v.as_object().ok_or_else(|| mlua::Error::runtime(format!("{path} must be an object")).into())
}

fn is_empty_object(v: &JsonValue) -> bool {
    v.as_object().map(|m| m.is_empty()).unwrap_or(false)
}

fn validate_obj_list_field(
    doc_obj: &serde_json::Map<String, JsonValue>, field: &str, item_validator: impl Fn(usize, &JsonValue) -> Result<(), LuaRuntimeError>,
) -> Result<(), LuaRuntimeError> {
    let Some(v) = doc_obj.get(field) else {
        return Ok(()); // not there -> muted
    };

    if is_empty_object(v) {
        return Ok(()); // {} -> muted
    }

    let arr = expect_array(v, &format!("doc.{field}"))?;
    for (i, item) in arr.iter().enumerate() {
        item_validator(i, item)?;
    }
    Ok(())
}

pub fn validate_module_doc(doc: &JsonValue) -> Result<(), LuaRuntimeError> {
    let obj = expect_object(doc, "doc")?;

    // required
    let name = obj.get("name").ok_or_else(|| mlua::Error::runtime("doc.name is required"))?;
    let _ = expect_string(name, "doc.name")?;

    let desc = obj.get("description").ok_or_else(|| mlua::Error::runtime("doc.description is required"))?;
    let _ = expect_string(desc, "doc.description")?;

    // optional strings
    if let Some(v) = obj.get("version") {
        let _ = expect_string(v, "doc.version")?;
    }
    if let Some(v) = obj.get("author") {
        let _ = expect_string(v, "doc.author")?;
    }

    // arguments: array of objects (muted if missing or {})
    validate_obj_list_field(obj, "arguments", |i, item| {
        let p = format!("doc.arguments[{i}]");
        let aobj = expect_object(item, &p)?;

        let n = aobj.get("name").ok_or_else(|| mlua::Error::runtime(format!("{p}.name is required")))?;
        let _ = expect_string(n, &format!("{p}.name"))?;

        if let Some(t) = aobj.get("type") {
            let _ = expect_string(t, &format!("{p}.type"))?;
        }
        if let Some(r) = aobj.get("required") {
            let _ = expect_bool(r, &format!("{p}.required"))?;
        }
        if let Some(d) = aobj.get("description") {
            let _ = expect_string(d, &format!("{p}.description"))?;
        }
        Ok(())
    })?;

    // options: array of objects (muted if missing or {})
    validate_obj_list_field(obj, "options", |i, item| {
        let p = format!("doc.options[{i}]");
        let oobj = expect_object(item, &p)?;

        let n = oobj.get("name").ok_or_else(|| mlua::Error::runtime(format!("{p}.name is required")))?;
        let _ = expect_string(n, &format!("{p}.name"))?;

        if let Some(d) = oobj.get("description") {
            let _ = expect_string(d, &format!("{p}.description"))?;
        }
        Ok(())
    })?;

    // examples: array of objects (muted if missing or {})
    validate_obj_list_field(obj, "examples", |i, item| {
        let p = format!("doc.examples[{i}]");
        let eobj = expect_object(item, &p)?;

        let code = eobj.get("code").ok_or_else(|| mlua::Error::runtime(format!("{p}.code is required")))?;
        let _ = expect_string(code, &format!("{p}.code"))?;

        if let Some(d) = eobj.get("description") {
            let _ = expect_string(d, &format!("{p}.description"))?;
        }
        Ok(())
    })?;

    // returns: object (optional). If present but {} -> muted too.
    if let Some(ret) = obj.get("returns")
        && !is_empty_object(ret) {
            let robj = expect_object(ret, "doc.returns")?;
            if let Some(d) = robj.get("description") {
                let _ = expect_string(d, "doc.returns.description")?;
            }
            // sample: any JSON type ok
        }

    Ok(())
}
