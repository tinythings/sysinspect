use std::{env, path::PathBuf};

/// Default shared library root for Sysinspect artefacts.
pub static DEFAULT_SHARELIB_ROOT: &str = "/usr/share/sysinspect";

/// Relative path to MeNotify Lua scripts under the shared library root.
pub static MENOTIFY_LUA_ROOT: &str = "lib/sensors/lua";

/// Relative path to MeNotify shared Lua libraries.
pub static MENOTIFY_LUA_SITE_ROOT: &str = "lib/sensors/lua/site-lua";

/// Returns the configured shared library root for MeNotify.
///
/// # Returns
///
/// Returns a `PathBuf` pointing to the shared library root. If the
/// `SYSINSPECT_SHARELIB_ROOT` environment variable is set and non-empty, it is
/// used. Otherwise the default root is returned.
pub fn get_sharelib_root() -> PathBuf {
    env::var("SYSINSPECT_SHARELIB_ROOT")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SHARELIB_ROOT))
}

/// Returns the absolute root directory where MeNotify Lua scripts live.
///
/// # Arguments
///
/// * `sharelib_root` - Shared library root for Sysinspect artefacts.
///
/// # Returns
///
/// Returns a `PathBuf` pointing to the directory containing MeNotify Lua entry
/// scripts.
pub fn get_script_root(sharelib_root: &std::path::Path) -> PathBuf {
    sharelib_root.join(MENOTIFY_LUA_ROOT)
}

/// Returns the absolute root directory where MeNotify shared Lua libraries live.
///
/// # Arguments
///
/// * `sharelib_root` - Shared library root for Sysinspect artefacts.
///
/// # Returns
///
/// Returns a `PathBuf` pointing to the directory containing reusable MeNotify
/// Lua libraries.
pub fn get_site_root(sharelib_root: &std::path::Path) -> PathBuf {
    sharelib_root.join(MENOTIFY_LUA_SITE_ROOT)
}

/// Returns a Lua `package.path` fragment for the given directory.
///
/// # Arguments
///
/// * `dir` - Directory to expose to Lua module resolution.
///
/// # Returns
///
/// Returns a string in the form `dir/?.lua;dir/?/init.lua`.
pub fn get_path_fragment(dir: &std::path::Path) -> String {
    let dir = dir.to_string_lossy();
    format!("{dir}/?.lua;{dir}/?/init.lua")
}
