//! CLI-facing planning types for `sysinspect network --add`.

mod artifact;
mod parser;
mod render;
mod types;
mod workflow;

#[cfg(test)]
pub(crate) use artifact::{ArtifactArch, ArtifactFamily, MinionCatalogue, PlatformId};
#[cfg(test)]
pub(crate) use parser::{normalise_host, normalise_path, parse, parse_entry, resolve_dest, resolve_remote_path};
pub(crate) use workflow::NetworkAddWorkflow;
#[cfg(test)]
pub(crate) use workflow::registration_mismatch_id;
