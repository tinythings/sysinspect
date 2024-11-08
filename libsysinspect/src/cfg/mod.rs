/*
Config reader
 */

pub mod mmconf;

use crate::SysinspectError;
use nix::unistd::Uid;
use std::{env, path::PathBuf};

pub const APP_CONF: &str = "sysinspect.conf";
pub const APP_DOTCONF: &str = ".sysinspect";

/// Select app conf
pub fn select_config(p: Option<PathBuf>) -> Result<PathBuf, SysinspectError> {
    // Override path from options
    if let Some(ovrp) = p {
        if ovrp.exists() {
            return Ok(ovrp);
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
    let cfp = PathBuf::from(format!("/etc/{}", APP_CONF));
    if cfp.exists() {
        return Ok(cfp);
    }

    Err(SysinspectError::ConfigError("No config has been found".to_string()))
}
