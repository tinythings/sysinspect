use std::path::PathBuf;

/// Error type for MeNotify runtime bootstrap and script resolution.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum MeNotifyError {
    /// Listener is missing a module suffix after `menotify.`
    #[error("listener '{0}' is missing a module name")]
    MissingModule(String),

    /// Listener family is not `menotify`
    #[error("listener '{0}' is not a menotify listener")]
    InvalidListener(String),

    /// Script path could not be found on disk.
    #[error("script '{module}' was not found at '{path}'")]
    MissingScript { module: String, path: PathBuf },
}
