use indexmap::IndexMap;
use serde_json::{Number, Value};

/// Parse a string into a serde_json::Value
fn get_json_value(s: &str) -> Value {
    let s = s.trim();

    // Null
    if s.eq_ignore_ascii_case("null") || s == "~" {
        return Value::Null;
    }
    // Bool
    if s.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    // Int (accepts only if string is identical to its integer representation)
    if let Ok(i) = s.parse::<i64>() {
        if s == i.to_string() {
            return Value::Number(Number::from(i));
        }
    }
    // Float (but not if it parses as int)
    if let Ok(f) = s.parse::<f64>() {
        if s.contains('.') {
            if let Some(n) = Number::from_f64(f) {
                return Value::Number(n);
            }
        }
    }
    // Quoted string
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Value::String(s[1..s.len() - 1].to_string());
    }

    // No clue, therefore just a string
    Value::String(s.to_string())
}

/// Get context data from a string
pub fn get_context(c: &str) -> Option<IndexMap<String, serde_json::Value>> {
    let c = c.trim();
    if c.is_empty() {
        return None;
    }

    let c: IndexMap<String, Value> = c
        .split(',')
        .filter_map(|p| {
            let mut d = p.splitn(2, ':');
            match (d.next(), d.next()) {
                (Some(k), Some(v)) => Some((k.trim().to_string(), get_json_value(v))),
                _ => None,
            }
        })
        .collect();

    if c.is_empty() {
        return None;
    }

    Some(c)
}
