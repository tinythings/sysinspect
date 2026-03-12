use std::path::PathBuf;

/// Error type for MeNotify runtime bootstrap and script resolution.
#[derive(thiserror::Error, Debug)]
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

    /// Script file could not be read from disk.
    #[error("failed to read script at '{path}': {source}")]
    ReadScript {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Lua runtime returned an error while loading or validating a script.
    #[error("lua runtime error: {0}")]
    Lua(#[from] mlua::Error),

    /// HTTP runtime returned an error while calling a remote endpoint.
    #[error("http runtime error: {0}")]
    Http(#[from] reqwest::Error),

    /// Script does not export a valid entrypoint.
    #[error("module '{0}' must export either tick(ctx) or loop(ctx)")]
    MissingEntrypoint(String),

    /// Script exports both tick(ctx) and loop(ctx), which is invalid in v1.
    #[error("module '{0}' exports both tick(ctx) and loop(ctx)")]
    AmbiguousEntrypoint(String),

    /// `ctx.emit` received invalid metadata.
    #[error("emit metadata is invalid: {0}")]
    InvalidEmitMeta(String),

    /// HTTP request specification is invalid.
    #[error("http request spec is invalid: {0}")]
    HttpSpec(String),

    /// PackageKit helper request is invalid or could not be converted.
    #[error("packagekit helper error: {0}")]
    PackageKit(String),
}

impl From<MeNotifyError> for libcommon::SysinspectError {
    fn from(value: MeNotifyError) -> Self {
        libcommon::SysinspectError::ModuleError(format!("MeNotify: {value}"))
    }
}
