use crate::error::MeNotifyError;
use std::path::{Path, PathBuf};

/// Parsed MeNotify listener reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeNotifyModuleRef {
    listener: String,
    module: String,
}

impl MeNotifyModuleRef {
    /// Parses a full listener string into a MeNotify module reference.
    ///
    /// # Arguments
    ///
    /// * `listener` - Full listener string, for example `menotify.foo`.
    ///
    /// # Returns
    ///
    /// Returns a parsed `MeNotifyModuleRef` on success, or `MeNotifyError`
    /// if the listener family or module suffix is invalid.
    pub fn new(listener: &str) -> Result<Self, MeNotifyError> {
        listener
            .split_once('.')
            .ok_or_else(|| MeNotifyError::MissingModule(listener.to_string()))
            .and_then(|(root, module)| {
                if root != "menotify" {
                    return Err(MeNotifyError::InvalidListener(listener.to_string()));
                }
                if module.trim().is_empty() {
                    return Err(MeNotifyError::MissingModule(listener.to_string()));
                }
                Ok(Self {
                    listener: listener.to_string(),
                    module: module.trim().to_string(),
                })
            })
    }

    /// Returns the full listener string.
    ///
    /// # Returns
    ///
    /// Returns the original full listener string.
    pub fn listener(&self) -> &str {
        &self.listener
    }

    /// Returns the resolved module name.
    ///
    /// # Returns
    ///
    /// Returns the module name suffix after `menotify.`.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Returns the expected script path under the provided script root.
    ///
    /// # Arguments
    ///
    /// * `script_root` - Root directory containing MeNotify Lua scripts.
    ///
    /// # Returns
    ///
    /// Returns a `PathBuf` pointing to `<script_root>/<module>.lua`.
    pub fn script_path(&self, script_root: &Path) -> PathBuf {
        script_root.join(format!("{}.lua", self.module))
    }
}
