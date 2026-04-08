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
#[cfg(test)]
pub(crate) use render::{render_outcomes, render_results};
#[cfg(test)]
pub(crate) use types::{AddHost, AddOutcome, AddStatus, HostOp};
pub(crate) use workflow::NetworkAddWorkflow;
#[cfg(test)]
pub(crate) use workflow::{
    BootstrapAttemptState, actionable_add_error, bootstrap_attempt_state, classify_destination_state, is_missing_master_minion,
    is_waitable_console_miss, managed_roots, marker_matches_managed_root, master_fingerprint_from_key_file, registration_mismatch_id,
    rows_have_traits, startup_sync_ready,
};
