use std::{fmt, path::PathBuf};

use libcommon::SysinspectError;

/// Error type for model browsing operations.
#[derive(Debug)]
pub enum ModelBrowseError {
    /// The model could not be loaded (wraps the underlying load error).
    LoadError(SysinspectError),
}

impl fmt::Display for ModelBrowseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelBrowseError::LoadError(e) => write!(f, "Model load error: {e}"),
        }
    }
}

impl std::error::Error for ModelBrowseError {}

impl From<SysinspectError> for ModelBrowseError {
    fn from(e: SysinspectError) -> Self {
        ModelBrowseError::LoadError(e)
    }
}

/// Model-level metadata extracted from the model spec header.
#[derive(Debug, Clone)]
pub struct BrowsedModelMetadata {
    /// Short identifier derived from the model directory name.
    pub id: String,
    /// Absolute path to the model directory.
    pub path: PathBuf,
    /// Model name from the header.
    pub name: String,
    /// Model version from the header.
    pub version: String,
    /// Human-readable description from the header.
    pub description: String,
    /// Maintainer string from the header.
    pub maintainer: String,
}

/// Severity level for a browse diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelBrowseDiagnosticLevel {
    Info,
    Warning,
    Error,
}

/// A single diagnostic message discovered during browsing.
#[derive(Debug, Clone)]
pub struct ModelBrowseDiagnostic {
    pub level: ModelBrowseDiagnosticLevel,
    pub message: String,
    /// Optional identifier for the source section or element.
    pub path: Option<String>,
}

/// Top-level browse result for one model.
///
/// This is the primary output that TUI, CLI, or other consumers
/// will use. It contains declared structure, not runtime execution
/// selections.
#[derive(Debug, Clone)]
pub struct BrowsedModel {
    pub metadata: BrowsedModelMetadata,
    pub entities: Vec<BrowsedEntity>,
    pub relations: Vec<BrowsedRelation>,
    pub entrypoints: Vec<BrowsedEntrypoint>,
    pub actions: Vec<BrowsedAction>,
    /// Deduplicated list of all declared action state keys.
    pub states: Vec<String>,
    pub diagnostics: Vec<ModelBrowseDiagnostic>,
}

/// A declared entity extracted from the model's `entities` section.
#[derive(Debug, Clone)]
pub struct BrowsedEntity {
    pub id: String,
    pub descr: String,
    pub inherits: Vec<String>,
    pub depends: Vec<String>,
    /// Outer claim-state keys (e.g. `$`, `baseline`, `verbose`).
    pub claim_state_keys: Vec<String>,
    /// Inner claim labels, unique across all states (e.g. `default`, `common`, `label`).
    pub claim_labels: Vec<String>,
}

/// A declared relation extracted from the model's `relations` section.
#[derive(Debug, Clone)]
pub struct BrowsedRelation {
    pub id: String,
    pub states: Vec<BrowsedRelationState>,
}

/// One state within a relation, listing required entities.
#[derive(Debug, Clone)]
pub struct BrowsedRelationState {
    pub state: String,
    pub required_entities: Vec<String>,
}

/// A user-facing entrypoint into a model.
#[derive(Debug, Clone)]
pub enum BrowsedEntrypoint {
    /// A checkbook label that references one or more relations.
    CheckbookLabel {
        label: String,
        relation_ids: Vec<String>,
        /// First-level entity IDs reachable through the referenced relations.
        entity_ids: Vec<String>,
    },
    /// A bare entity that can be targeted directly.
    Entity { id: String, descr: String },
}

/// A declared action extracted from the model's `actions` section.
#[derive(Debug, Clone)]
pub struct BrowsedAction {
    pub action_id: String,
    pub description: String,
    pub module: String,
    pub binds_to: Vec<String>,
    pub states: Vec<BrowsedActionState>,
}

/// One declared state within an action, carrying its parameter metadata.
#[derive(Debug, Clone)]
pub struct BrowsedActionState {
    pub state: String,
    pub opts: Vec<String>,
    pub args: Vec<(String, String)>,
    pub context_vars: Vec<(String, String, bool)>,
    pub conditions: Vec<(String, String)>,
}
