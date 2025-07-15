use indexmap::IndexMap;
use serde_json::{Number, Value};

/// Parse a string into a serde_json::Value
fn get_json_value(s: &str) -> Value {
    let trimmed = s.trim();

    // Null
    if trimmed.eq_ignore_ascii_case("null") || trimmed == "~" {
        return Value::Null;
    }
    // Bool
    if trimmed.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    // Int (accepts only if string is identical to its integer representation)
    if let Ok(i) = trimmed.parse::<i64>() {
        if trimmed == i.to_string() {
            return Value::Number(Number::from(i));
        }
    }
    // Float (but not if it parses as int)
    if let Ok(f) = trimmed.parse::<f64>() {
        if trimmed.contains('.') {
            if let Some(num) = Number::from_f64(f) {
                return Value::Number(num);
            }
        }
    }
    // Quoted string
    if (trimmed.starts_with('"') && trimmed.ends_with('"')) || (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
        return Value::String(trimmed[1..trimmed.len() - 1].to_string());
    }

    // No clue, therefore just a string
    Value::String(trimmed.to_string())
}

/// Get context data from a string
pub fn get_context(context: &str) -> Option<IndexMap<String, serde_json::Value>> {
    let context = context.trim();
    if context.is_empty() {
        return None;
    }

    let context: IndexMap<String, Value> = context
        .split(',')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, ':');
            match (parts.next(), parts.next()) {
                (Some(k), Some(v)) => Some((k.trim().to_string(), get_json_value(v))),
                _ => None,
            }
        })
        .collect();

    if context.is_empty() {
        return None;
    }

    Some(context)
}
