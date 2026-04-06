use std::path::PathBuf;

/// Requested host lifecycle operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostOp {
    Add,
    Remove,
}

impl HostOp {
    pub(crate) fn progress_label(self) -> &'static str {
        if self == Self::Add { "Auto-add: onboarding" } else { "Auto-remove: handling" }
    }

    pub(crate) fn summary_label(self) -> &'static str {
        if self == Self::Add { "Planned onboarding" } else { "Planned removal" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AddStatus {
    Pending,
    Online,
    Removed,
    Failed,
    Absent,
}

impl AddStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Pending => "-",
            Self::Online => "online",
            Self::Removed => "removed",
            Self::Failed => "failed",
            Self::Absent => "absent",
        }
    }
}

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
    pub(crate) op: HostOp,
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
    pub(crate) display_path: String,
    pub(crate) platform: String,
    pub(crate) status: AddStatus,
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
