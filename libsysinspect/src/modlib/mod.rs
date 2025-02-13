use indexmap::IndexMap;
use shlex::Shlex;

pub mod modinit;
pub mod response;
pub mod runtime;
pub mod tpl;

/// Parse passed environment.
/// Env is passed in the form of key=value. The following form is supported:
///
/// `VAR_ONE="value" VAR_TWO=value VAR_THREE="spaces are supported"`
pub fn getenv(env: &str) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for elm in Shlex::new(env) {
        if let Some(pos) = elm.find('=') {
            out.insert(elm[..pos].to_string(), elm[pos + 1..].to_string().trim_matches('"').to_string());
        }
    }

    out
}
