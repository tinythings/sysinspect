//! CLI-facing planning types for `sysinspect network --add`.

mod parser;
mod render;
mod types;
mod workflow;

pub(crate) use parser::{parse, resolve_remote_path};
pub(crate) use workflow::NetworkAddWorkflow;
