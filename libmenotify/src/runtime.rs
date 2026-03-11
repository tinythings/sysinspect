use crate::{
    error::MeNotifyError,
    layout::{get_script_root, get_sharelib_root, get_site_root},
    module::MeNotifyModuleRef,
    program::MeNotifyProgram,
};
use colored::Colorize;
use std::path::{Path, PathBuf};

/// Runtime bootstrap for one configured MeNotify sensor instance.
#[derive(Debug, Clone)]
pub struct MeNotifyRuntime {
    sid: String,
    module_ref: Option<MeNotifyModuleRef>,
    listener: String,
    sharelib_root: PathBuf,
}

impl MeNotifyRuntime {
    /// Creates a new MeNotify runtime bootstrap object.
    ///
    /// # Arguments
    ///
    /// * `sid` - Sensor id from the DSL.
    /// * `listener` - Full listener string, for example `menotify.foo`.
    ///
    /// # Returns
    ///
    /// Returns a `MeNotifyRuntime` ready to resolve script and library paths.
    pub fn new(sid: String, listener: String) -> Self {
        Self::with_sharelib_root(sid, listener, get_sharelib_root())
    }

    /// Creates a new MeNotify runtime bootstrap object with an explicit sharelib root.
    ///
    /// # Arguments
    ///
    /// * `sid` - Sensor id from the DSL.
    /// * `listener` - Full listener string, for example `menotify.foo`.
    /// * `sharelib_root` - Shared library root to use for script lookup.
    ///
    /// # Returns
    ///
    /// Returns a `MeNotifyRuntime` ready to resolve script and library paths.
    pub fn with_sharelib_root(sid: String, listener: String, sharelib_root: PathBuf) -> Self {
        Self { sid, module_ref: MeNotifyModuleRef::new(&listener).ok(), listener, sharelib_root }
    }

    /// Returns the sensor id.
    ///
    /// # Returns
    ///
    /// Returns the DSL sensor id.
    pub fn sid(&self) -> &str {
        &self.sid
    }

    /// Returns the full listener string.
    ///
    /// # Returns
    ///
    /// Returns the listener string used to create the runtime.
    pub fn listener(&self) -> &str {
        &self.listener
    }

    /// Returns the resolved module name, if available.
    ///
    /// # Returns
    ///
    /// Returns `Some(&str)` with the parsed module name, or `None` if the
    /// listener is missing a module suffix.
    pub fn module_name(&self) -> Option<&str> {
        self.module_ref.as_ref().map(MeNotifyModuleRef::module)
    }

    /// Returns the shared library root.
    ///
    /// # Returns
    ///
    /// Returns the absolute shared library root used by this runtime.
    pub fn sharelib_root(&self) -> &Path {
        &self.sharelib_root
    }

    /// Returns the script root for MeNotify Lua entry scripts.
    ///
    /// # Returns
    ///
    /// Returns the absolute root directory where MeNotify scripts are expected.
    pub fn script_root(&self) -> PathBuf {
        get_script_root(self.sharelib_root())
    }

    /// Returns the site library root for shared MeNotify Lua code.
    ///
    /// # Returns
    ///
    /// Returns the absolute root directory for shared MeNotify Lua libraries.
    pub fn site_root(&self) -> PathBuf {
        get_site_root(self.sharelib_root())
    }

    /// Returns the expected entry script path for the resolved module.
    ///
    /// # Returns
    ///
    /// Returns `Ok(PathBuf)` for the script path, or `MeNotifyError` if the
    /// listener does not contain a valid module name.
    pub fn script_path(&self) -> Result<PathBuf, MeNotifyError> {
        self.module_ref
            .as_ref()
            .map(|module| module.script_path(&self.script_root()))
            .ok_or_else(|| MeNotifyError::MissingModule(self.listener.clone()))
    }

    /// Verifies that the resolved script file exists on disk.
    ///
    /// # Returns
    ///
    /// Returns `Ok(PathBuf)` with the resolved script path if it exists, or
    /// `MeNotifyError` if the listener is invalid or the file is missing.
    pub fn require_script(&self) -> Result<PathBuf, MeNotifyError> {
        self.script_path().and_then(|path| {
            path.exists()
                .then_some(path.clone())
                .ok_or_else(|| MeNotifyError::MissingScript { module: self.module_name().unwrap_or_default().to_string(), path })
        })
    }

    /// Loads and validates the configured Lua script.
    ///
    /// # Returns
    ///
    /// Returns a loaded `MeNotifyProgram` if the script exists and exports a
    /// valid MeNotify contract.
    pub fn load_program(&self) -> Result<MeNotifyProgram, MeNotifyError> {
        MeNotifyProgram::new(self)
    }

    /// Logs the current bootstrap state for the stub runtime.
    ///
    /// # Arguments
    ///
    /// * `err` - Bootstrap error to render into the main log stream.
    ///
    /// # Returns
    ///
    /// Returns nothing. This method only emits a log record for the given
    /// bootstrap error.
    pub fn log_bootstrap_error(&self, err: &MeNotifyError) {
        match err {
            MeNotifyError::MissingModule(_) => log::error!(
                "[{}] '{}' started without a module name in listener '{}'; sensor is a stub and will stay idle",
                "menotify".bright_magenta(),
                self.sid,
                self.listener
            ),
            MeNotifyError::MissingScript { path, .. } => log::error!(
                "[{}] '{}' expects script '{}' for listener '{}'; runtime is not implemented yet, sensor stays idle",
                "menotify".bright_magenta(),
                self.sid,
                path.display(),
                self.listener
            ),
            MeNotifyError::ReadScript { path, source } => log::error!(
                "[{}] '{}' failed reading script '{}' for listener '{}': {}",
                "menotify".bright_magenta(),
                self.sid,
                path.display(),
                self.listener,
                source
            ),
            MeNotifyError::MissingEntrypoint(module) => {
                log::error!("[{}] '{}' loaded module '{}' but it exports no valid entrypoint", "menotify".bright_magenta(), self.sid, module)
            }
            MeNotifyError::AmbiguousEntrypoint(module) => {
                log::error!("[{}] '{}' loaded module '{}' but it exports both tick(ctx) and loop(ctx)", "menotify".bright_magenta(), self.sid, module)
            }
            MeNotifyError::Lua(err) => {
                log::error!("[{}] '{}' failed to bootstrap Lua for listener '{}': {}", "menotify".bright_magenta(), self.sid, self.listener, err)
            }
            MeNotifyError::InvalidListener(_) => log::error!(
                "[{}] '{}' got invalid listener '{}'; sensor is a stub and will stay idle",
                "menotify".bright_magenta(),
                self.sid,
                self.listener
            ),
            MeNotifyError::InvalidEmitMeta(err) => {
                log::error!("[{}] '{}' got invalid emit metadata in listener '{}': {}", "menotify".bright_magenta(), self.sid, self.listener, err)
            }
        }
    }

    /// Logs the current bootstrap state for the stub runtime.
    ///
    /// # Returns
    ///
    /// Returns nothing. This method only emits log records describing what the
    /// runtime resolved or failed to resolve.
    pub fn run_stub(&self) {
        match self.load_program() {
            Ok(program) => log::warn!(
                "[{}] '{}' validated module '{}' at '{}' with '{:?}' entrypoint; host API is not implemented yet, sensor stays idle",
                "menotify".bright_magenta(),
                self.sid,
                program.module_name(),
                program.script_path().display(),
                program.contract().entrypoint()
            ),
            Err(err) => self.log_bootstrap_error(&err),
        }
    }
}
