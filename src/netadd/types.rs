use std::path::PathBuf;

/// Parsed host onboarding input before defaults are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostSpec {
    pub(crate) raw: String,
    pub(crate) user: Option<String>,
    pub(crate) host: String,
    pub(crate) path: Option<String>,
}

/// One validated onboarding request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddRequest {
    pub(crate) hosts: Vec<HostSpec>,
    pub(crate) user: String,
}

/// One fully resolved onboarding target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddHost {
    pub(crate) raw: String,
    pub(crate) host: String,
    pub(crate) host_norm: String,
    pub(crate) user: String,
    pub(crate) path: Option<String>,
    pub(crate) path_norm: Option<String>,
}

/// One validated onboarding plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddPlan {
    pub(crate) items: Vec<AddHost>,
}

/// One operator-visible onboarding outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddOutcome {
    pub(crate) host: AddHost,
    pub(crate) state: &'static str,
    pub(crate) detail: String,
}

/// One canonical deduplication key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AddKey {
    pub(crate) user: String,
    pub(crate) host: String,
    pub(crate) path: Option<String>,
}

/// One resolved remote destination path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedDest {
    pub(crate) input: Option<String>,
    pub(crate) path: Option<PathBuf>,
}
