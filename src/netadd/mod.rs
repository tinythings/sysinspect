//! CLI-facing planning types for `sysinspect network --add`.

mod parser;
mod render;
mod types;
mod workflow;

#[cfg(test)]
pub(crate) use parser::{normalise_host, normalise_path, parse, parse_entry, resolve_dest, resolve_remote_path};
pub(crate) use workflow::NetworkAddWorkflow;
