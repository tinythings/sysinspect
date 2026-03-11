use crate::MeNotifyError;
use mlua::{Function, Table};

/// Declares which entrypoint style a MeNotify script exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeNotifyEntrypoint {
    Tick,
    Loop,
}

/// Validated callable contract for one MeNotify script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeNotifyContract {
    entrypoint: MeNotifyEntrypoint,
}

impl MeNotifyContract {
    /// Validates the exported Lua module table and resolves its entrypoint.
    ///
    /// # Arguments
    ///
    /// * `module` - Lua module table returned by evaluating the script.
    /// * `module_name` - Logical module name used for diagnostics.
    ///
    /// # Returns
    ///
    /// Returns a validated `MeNotifyContract` if the script exports exactly one
    /// supported entrypoint.
    pub fn new(module: &Table, module_name: &str) -> Result<Self, MeNotifyError> {
        match (module.get::<Option<Function>>("tick")?, module.get::<Option<Function>>("loop")?) {
            (Some(_), None) => Ok(Self { entrypoint: MeNotifyEntrypoint::Tick }),
            (None, Some(_)) => Ok(Self { entrypoint: MeNotifyEntrypoint::Loop }),
            (Some(_), Some(_)) => Err(MeNotifyError::AmbiguousEntrypoint(module_name.to_string())),
            (None, None) => Err(MeNotifyError::MissingEntrypoint(module_name.to_string())),
        }
    }

    /// Returns the validated entrypoint mode.
    ///
    /// # Returns
    ///
    /// Returns the `MeNotifyEntrypoint` selected during validation.
    pub fn entrypoint(&self) -> MeNotifyEntrypoint {
        self.entrypoint
    }
}
