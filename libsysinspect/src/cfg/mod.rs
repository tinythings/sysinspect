/*
Config reader
 */

pub mod mmconf;

use crate::SysinspectError;
use mmconf::MinionConfig;
use nix::unistd::Uid;
use once_cell::sync::OnceCell;
use std::{env, path::PathBuf};

pub const APP_CONF: &str = "sysinspect.conf";
pub const APP_DOTCONF: &str = ".sysinspect";
pub const APP_HOME: &str = "/etc/sysinspect";

/// Select app conf
pub fn select_config_path(p: Option<&str>) -> Result<PathBuf, SysinspectError> {
    // Override path from options
    if let Some(ovrp) = p {
        if !ovrp.is_empty() {
            let ovrp = PathBuf::from(ovrp);
            if ovrp.exists() {
                return Ok(ovrp);
            }

            log::warn!("Preferred config at {} does not exist, falling back", ovrp.to_str().unwrap_or_default());
        }
    }

    // Current
    let cfp: PathBuf = env::current_dir()?.canonicalize()?.join(APP_CONF);
    if cfp.exists() {
        return Ok(cfp);
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

/// Minion Confinguration
static _MINION_CFG: OnceCell<MinionConfig> = OnceCell::new();

/// Returns a copy of initialised traits.
pub fn get_minion_config(p: Option<&str>) -> Result<MinionConfig, SysinspectError> {
    log::debug!("Getting minion config");
    Ok(_MINION_CFG.get_or_try_init(|| MinionConfig::new(select_config_path(p)?))?.to_owned())
}
