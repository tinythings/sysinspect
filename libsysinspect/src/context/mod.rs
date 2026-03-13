//! Shared context parsing and small request payload types used by CLI and console paths.

pub mod host;

#[cfg(test)]
mod host_ut;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
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
    if let Ok(i) = s.parse::<i64>()
        && s == i.to_string()
    {
        return Value::Number(Number::from(i));
    }
    // Float (but not if it parses as int)
    if let Ok(f) = s.parse::<f64>()
        && s.contains('.')
        && let Some(n) = Number::from_f64(f)
    {
        return Value::Number(n);
    }
    // Quoted string
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Value::String(s[1..s.len() - 1].to_string());
    }

    // No clue, therefore just a string
    Value::String(s.to_string())
}

/// Parse comma-separated `key:value` pairs into a typed JSON map.
///
/// Values are interpreted as JSON-like scalars where possible:
/// `null`, booleans, integers, floats, and quoted strings. Everything
/// else is kept as a plain string.
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

/// Parse comma-separated keys from a string.
pub fn get_context_keys(c: &str) -> Vec<String> {
    c.trim().split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// Console request payload used for profile management operations.
pub struct ProfileConsoleRequest {
    op: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    matches: Vec<String>,
    #[serde(default)]
    library: bool,
    #[serde(default)]
    profiles: Vec<String>,
}

impl ProfileConsoleRequest {
    /// Parse a profile console request from the JSON context payload.
    pub fn from_context(context: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(context)
    }

    /// Return the requested profile operation name.
    pub fn op(&self) -> &str {
        &self.op
    }

    /// Return the target profile name, if present.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the module or library selector list carried by the request.
    pub fn matches(&self) -> &[String] {
        &self.matches
    }

    /// Return whether the request targets library selectors instead of module selectors.
    pub fn library(&self) -> bool {
        self.library
    }

    /// Return the profile names carried by tag or untag requests.
    pub fn profiles(&self) -> &[String] {
        &self.profiles
    }
}
