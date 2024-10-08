// Very simple interpolator

use regex::Regex;
use std::collections::HashMap;

/// Interpolate a template with given variables with the following syntax:
///
/// ```bash
///    $(my.whatever.variable)
/// ```
///
/// This allows to interpolate various patterns, for example:
///
/// ```bash
///    string = "Hi, $(name)"
/// ```
pub fn interpolate(tpl: &str, vars: &HashMap<String, String>) -> String {
    Regex::new(r"\$\((\w+)\)")
        .unwrap()
        .replace_all(tpl, |caps: &regex::Captures| {
            let var_name = &caps[1];
            vars.get(var_name)
                .map_or(caps[0].to_string(), |v| v.to_string())
        })
        .into_owned()
}

/// Get a list of all variables, mentioned in a template. For example,
/// the following template will return a list of them:
///
/// ```text
///   foo bar $(baz) toto $(spam) here
/// ```
///
/// The following list of variables will be returned:
/// - `baz`
/// - `spam`
pub fn extract(tpl: &str) -> Vec<String> {
    Regex::new(r"\$\(([^)]+)\)")
        .unwrap()
        .captures_iter(tpl)
        .map(|cap| cap[1].to_string())
        .collect()
}
