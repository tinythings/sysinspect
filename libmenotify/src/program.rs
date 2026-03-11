use crate::{
    contract::MeNotifyContract,
    error::MeNotifyError,
    layout::get_path_fragment,
    runtime::MeNotifyRuntime,
};
use mlua::{Lua, Table};
use std::{fs, path::Path};

/// Loaded MeNotify Lua program bound to one configured sensor instance.
#[derive(Debug)]
pub struct MeNotifyProgram {
    contract: MeNotifyContract,
    lua: Lua,
    module_name: String,
    script_path: std::path::PathBuf,
}

impl MeNotifyProgram {
    /// Loads, evaluates, and validates one MeNotify Lua script.
    ///
    /// # Arguments
    ///
    /// * `runtime` - Runtime bootstrap with resolved listener and path state.
    ///
    /// # Returns
    ///
    /// Returns a loaded `MeNotifyProgram` if the script file exists and exports
    /// a valid MeNotify entrypoint.
    pub fn new(runtime: &MeNotifyRuntime) -> Result<Self, MeNotifyError> {
        runtime.require_script().and_then(|script_path| {
            fs::read_to_string(&script_path)
                .map_err(|source| MeNotifyError::ReadScript {
                    path: script_path.clone(),
                    source,
                })
                .and_then(|code| {
                    let lua = Lua::new();
                    Self::configure_path(&lua, runtime.script_root().as_path(), runtime.site_root().as_path())?;
                    let module: Table = lua.load(&code).set_name(runtime.listener()).eval()?;
                    Ok(Self {
                        contract: MeNotifyContract::new(&module, runtime.module_name().unwrap_or_default())?,
                        lua,
                        module_name: runtime.module_name().unwrap_or_default().to_string(),
                        script_path,
                    })
                })
        })
    }

    /// Configures Lua package resolution for MeNotify scripts.
    ///
    /// # Arguments
    ///
    /// * `lua` - Lua VM to configure.
    /// * `script_root` - Root directory for entry scripts.
    /// * `site_root` - Root directory for shared Lua libraries.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the Lua package configuration succeeds.
    pub fn configure_path(lua: &Lua, script_root: &Path, site_root: &Path) -> Result<(), MeNotifyError> {
        let package: mlua::Table = lua.globals().get("package")?;
        package.set("cpath", "")?;
        package.set("path", format!("{};{}", get_path_fragment(script_root), get_path_fragment(site_root)))?;
        Ok(())
    }

    /// Returns the validated contract.
    ///
    /// # Returns
    ///
    /// Returns the script contract selected during validation.
    pub fn contract(&self) -> MeNotifyContract {
        self.contract
    }

    /// Returns the loaded module name.
    ///
    /// # Returns
    ///
    /// Returns the logical module name used to load this script.
    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    /// Returns the resolved script path.
    ///
    /// # Returns
    ///
    /// Returns the absolute path to the loaded script file.
    pub fn script_path(&self) -> &Path {
        &self.script_path
    }

    /// Returns the underlying Lua VM.
    ///
    /// # Returns
    ///
    /// Returns a shared reference to the embedded Lua VM.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}
