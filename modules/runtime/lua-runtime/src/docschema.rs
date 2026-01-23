use crate::lrt::LuaRuntimeError;
use libmodcore::rtspec::{RuntimeModuleDocPrefix, RuntimeModuleDocumentation, RuntimeSpec};
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

    let arr = expect_array(v, &format!("{}.{}", RuntimeSpec::DocumentationFunction, field))?;
    for (i, item) in arr.iter().enumerate() {
        item_validator(i, item)?;
    }
    Ok(())
}

pub fn validate_module_doc(doc: &JsonValue) -> Result<(), LuaRuntimeError> {
    let obj = expect_object(doc, &RuntimeSpec::DocumentationFunction.to_string())?;
    let p = RuntimeModuleDocPrefix::new(&RuntimeSpec::DocumentationFunction);

    // required
    let name = obj
        .get(&RuntimeModuleDocumentation::Name.to_string())
        .ok_or_else(|| mlua::Error::runtime(format!("{} is required", p.field(&RuntimeModuleDocumentation::Name))))?;
    let _ = expect_string(name, &p.field(&RuntimeModuleDocumentation::Name))?;

    let desc = obj
        .get(&RuntimeModuleDocumentation::Description.to_string())
        .ok_or_else(|| mlua::Error::runtime(format!("{} is required", p.field(&RuntimeModuleDocumentation::Description))))?;
    let _ = expect_string(desc, &p.field(&RuntimeModuleDocumentation::Description))?;

    // optional strings
    if let Some(v) = obj.get(&RuntimeModuleDocumentation::Version.to_string()) {
        let _ = expect_string(v, &p.field(&RuntimeModuleDocumentation::Version))?;
    }
    if let Some(v) = obj.get(&RuntimeModuleDocumentation::Author.to_string()) {
        let _ = expect_string(v, &p.field(&RuntimeModuleDocumentation::Author))?;
    }

    // arguments: array of objects (muted if missing or {})
    validate_obj_list_field(obj, &RuntimeModuleDocumentation::Arguments.to_string(), |i, item| {
        let p = format!("{}.{}[{i}]", RuntimeSpec::DocumentationFunction, RuntimeModuleDocumentation::Arguments);
        let aobj = expect_object(item, &p)?;

        let n = aobj
            .get(&RuntimeModuleDocumentation::Name.to_string())
            .ok_or_else(|| mlua::Error::runtime(format!("{p}.{} is required", RuntimeModuleDocumentation::Name)))?;
        let _ = expect_string(n, &format!("{p}.{}", RuntimeModuleDocumentation::Name))?;

        if let Some(t) = aobj.get(&RuntimeModuleDocumentation::Type.to_string()) {
            let _ = expect_string(t, &format!("{p}.{}", RuntimeModuleDocumentation::Type))?;
        }
        if let Some(r) = aobj.get(&RuntimeModuleDocumentation::Required.to_string()) {
            let _ = expect_bool(r, &format!("{p}.{}", RuntimeModuleDocumentation::Required))?;
        }
        if let Some(d) = aobj.get(&RuntimeModuleDocumentation::Description.to_string()) {
            let _ = expect_string(d, &format!("{p}.{}", RuntimeModuleDocumentation::Description))?;
        }
        Ok(())
    })?;

    // options: array of objects (muted if missing or {})
    validate_obj_list_field(obj, &RuntimeModuleDocumentation::Options.to_string(), |i, item| {
        let p = format!("{}.{}[{i}]", RuntimeSpec::DocumentationFunction, RuntimeModuleDocumentation::Options);
        let oobj = expect_object(item, &p)?;

        let n = oobj
            .get(&RuntimeModuleDocumentation::Name.to_string())
            .ok_or_else(|| mlua::Error::runtime(format!("{p}.{} is required", RuntimeModuleDocumentation::Name)))?;
        let _ = expect_string(n, &format!("{p}.{}", RuntimeModuleDocumentation::Name))?;

        if let Some(d) = oobj.get(&RuntimeModuleDocumentation::Description.to_string()) {
            let _ = expect_string(d, &format!("{p}.{}", RuntimeModuleDocumentation::Description))?;
        }
        Ok(())
    })?;

    // examples: array of objects (muted if missing or {})
    validate_obj_list_field(obj, &RuntimeModuleDocumentation::Examples.to_string(), |i, item| {
        let p = format!("{}.{}[{i}]", RuntimeSpec::DocumentationFunction, RuntimeModuleDocumentation::Examples);
        let eobj = expect_object(item, &p)?;

        let code = eobj
            .get(&RuntimeModuleDocumentation::Code.to_string())
            .ok_or_else(|| mlua::Error::runtime(format!("{p}.{} is required", RuntimeModuleDocumentation::Code)))?;
        let _ = expect_string(code, &format!("{p}.{}", RuntimeModuleDocumentation::Code))?;

        if let Some(d) = eobj.get(&RuntimeModuleDocumentation::Description.to_string()) {
            let _ = expect_string(d, &format!("{p}.{}", RuntimeModuleDocumentation::Description))?;
        }
        Ok(())
    })?;

    // returns: object (optional). If present but {} -> muted too.
    if let Some(ret) = obj.get(&RuntimeModuleDocumentation::Returns.to_string())
        && !is_empty_object(ret)
    {
        let robj = expect_object(ret, &format!("{}.{}", RuntimeSpec::DocumentationFunction, RuntimeModuleDocumentation::Returns))?;
        if let Some(d) = robj.get(&RuntimeModuleDocumentation::Description.to_string()) {
            let _ = expect_string(
                d,
                &format!(
                    "{}.{}.{}",
                    RuntimeSpec::DocumentationFunction,
                    RuntimeModuleDocumentation::Returns,
                    RuntimeModuleDocumentation::Description
                ),
            )?;
        }
        // sample: any JSON type ok
    }

    Ok(())
}
