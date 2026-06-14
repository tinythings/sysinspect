/*
Config reader
 */

pub mod mmconf;
#[cfg(test)]
mod mmconf_ut;

use libcommon::SysinspectError;
use mmconf::MinionConfig;
use nix::unistd::Uid;
use serde_yaml::Value;
use std::{env, path::PathBuf};

pub const APP_CONF: &str = "sysinspect.conf";
pub const APP_DOTCONF: &str = ".sysinspect";
pub const APP_HOME: &str = "/etc/sysinspect";

/// Select app conf
pub fn select_config_path(p: Option<&str>) -> Result<PathBuf, SysinspectError> {
    // Override path from options
    if let Some(ovrp) = p
        && !ovrp.is_empty()
    {
        let ovrp = PathBuf::from(ovrp);
        if ovrp.exists() {
            return Ok(ovrp);
        }

        log::warn!("Preferred config at {} does not exist, falling back", ovrp.to_str().unwrap_or_default());
    }

    // Current
    let cfp: PathBuf = env::current_dir()?.canonicalize()?.join(APP_CONF);
    if cfp.exists() {
        return Ok(cfp);
    }

    // Self-contained layout: ../etc/sysinspect.conf relative to binary
    if let Ok(exe) = std::env::current_exe()
        && let Some(grandparent) = exe.parent().and_then(|p| p.parent())
    {
        let cfp = grandparent.join("etc").join(APP_CONF);
        if cfp.exists() {
            return Ok(cfp);
        }
    }

    // Dot-file
    let cfp = env::var_os("HOME").map(PathBuf::from).or_else(|| {
        #[cfg(unix)]
        {
            Some(PathBuf::from(format!("/home/{}", Uid::current())))
        }
    });
    if let Some(cfp) = cfp {
        let cfp = cfp.join(APP_DOTCONF);
        if cfp.exists() {
            return Ok(cfp);
        }
    }

    // Global conf
    let cfp = PathBuf::from(format!("{APP_HOME}/{APP_CONF}"));
    if cfp.exists() {
        return Ok(cfp);
    }

    Err(SysinspectError::ConfigError("No config has been found".to_string()))
}

/// Returns a copy of initialised traits.
pub fn get_minion_config(p: Option<&str>) -> Result<MinionConfig, SysinspectError> {
    log::debug!("Getting minion config");
    MinionConfig::new(select_config_path(p)?)
}

/// Derive the drop-in directory path alongside a config file.
pub(crate) fn dropins_dir(config_path: &std::path::Path) -> PathBuf {
    let fname = config_path.file_stem().unwrap_or_default();
    let mut dot_d = fname.to_os_string();
    dot_d.push(".d");
    config_path.with_file_name(dot_d)
}

/// Load and parse YAML drop-in files from a directory, sorted by filename.
pub(crate) fn load_dropins(dir: &std::path::Path) -> Vec<Value> {
    let mut values = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return values;
    };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()).map(|e| e == "yml" || e == "yaml").unwrap_or(false))
        .collect();
    files.sort();
    for f in &files {
        if let Ok(s) = std::fs::read_to_string(f)
            && let Ok(v) = serde_yaml::from_str::<Value>(&s)
        {
            values.push(v);
        }
    }
    values
}
