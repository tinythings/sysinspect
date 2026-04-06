use colored::Colorize;
use libcommon::SysinspectError;
use libsysinspect::cfg::mmconf::{
    CFG_AUTOSYNC_SHALLOW, CFG_SENSORS_ROOT, DEFAULT_MODULES_DIR, DEFAULT_MODULES_LIB_DIR, DEFAULT_MODULES_SHARELIB, MinionConfig, SysInspectConfig,
};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

use crate::local_marker::LocalMarker;

#[derive(Default)]
pub struct MinionSetup {
    alt_dir: String,
    sharelib: String,
    master_addr: String,
    cfg: MinionConfig,
}

impl MinionSetup {
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    /// Check if the directory is writable
    fn is_dir_w(path: &Path) -> bool {
        let tfp = path.join(format!(".tmp_wt-{}", Uuid::new_v4()));
        match File::create(&tfp) {
            Ok(mut file) => {
                let r = file.write_all(b"t");
                let _ = std::fs::remove_file(&tfp);
                r.is_ok()
            }
            Err(_) => false,
        }
    }

    fn get_sharelib(&self) -> &str {
        if self.sharelib.is_empty() {
            return DEFAULT_MODULES_SHARELIB;
        }
        self.sharelib.as_str()
    }

    fn get_shared_subdir(&self, dir: &str) -> String {
        format!("{}/{}", self.get_sharelib(), dir)
    }

    fn writable_parent(path: &Path) -> &Path {
        if path.exists() { path } else { path.parent().unwrap_or(path) }
    }

    fn check_my_permissions() -> Result<(), SysinspectError> {
        if unsafe { libc::getuid() } != 0 {
            return Err(SysinspectError::ConfigError("SysMinion must be run as root".to_string()));
        }
        Ok(())
    }

    /// Check directory structure
    fn check_dir_structure(&self) -> Result<(), SysinspectError> {
        if !self.alt_dir.is_empty() && !Path::new(&self.alt_dir).exists() {
            return Err(SysinspectError::ConfigError(format!("Given alternative directory {} does not exist", self.alt_dir)));
        }

        if !self.alt_dir.is_empty() && !Self::is_dir_w(Path::new(&self.alt_dir)) {
            return Err(SysinspectError::ConfigError(format!("Given alternative directory {} appears to be read-only", self.alt_dir)));
        }

        if self.alt_dir.is_empty() {
            // This is the scenario when SysInspect is not packaged as a system package
            for d in [
                Self::writable_parent(Path::new(self.get_sharelib())),
                Self::writable_parent(&self.cfg.install_bin_dir()),
                Self::writable_parent(&self.cfg.config_dir()),
                Self::writable_parent(&self.cfg.managed_pidfile_dir()),
                Self::writable_parent(&self.cfg.managed_tmp_dir()),
                Self::writable_parent(&self.cfg.managed_db_dir()),
            ] {
                if !Self::is_dir_w(d) {
                    return Err(SysinspectError::ConfigError(format!("Directory {} appears to be read-only", d.display())));
                }
            }
        }

        Ok(())
    }

    /// Generate directory structure
    fn generate_dir_structure(&self) -> Result<(), SysinspectError> {
        let dirs = [
            self.cfg.config_dir().display().to_string(),
            self.cfg.install_bin_dir().display().to_string(),
            self.cfg.managed_pidfile_dir().display().to_string(),
            self.cfg.managed_tmp_dir().display().to_string(),
            self.cfg.managed_db_dir().display().to_string(),
            self.cfg.models_dir().display().to_string(),
            self.cfg.functions_dir().display().to_string(),
            self.cfg.traits_dir().display().to_string(),
            self.cfg.sensors_dir().display().to_string(),
            self.cfg.profiles_dir().display().to_string(),
            self.cfg.transport_root().display().to_string(),
            self.cfg.transport_master_root().display().to_string(),
            self.get_shared_subdir(DEFAULT_MODULES_DIR),
            self.get_shared_subdir(DEFAULT_MODULES_LIB_DIR),
            self.get_shared_subdir(CFG_SENSORS_ROOT),
        ];

        for d in dirs {
            if !Path::new(&d).exists() {
                log::info!("Creating missing directory at {}", d.bright_white().bold());
                std::fs::create_dir_all(&d).map_err(|_| SysinspectError::ConfigError(format!("Unable to create directory {d}")))?;
            }
        }

        ensure_minion_tree(&self.cfg)?;

        Ok(())
    }

    /// Generate configuration files
    fn generate_config(&mut self) -> Result<(), SysinspectError> {
        #[allow(clippy::unnecessary_to_owned)]
        self.cfg.set_sharelib_path(&self.get_sharelib().to_string());
        self.cfg.set_pid_path(self.cfg.managed_pidfile_path().to_str().unwrap_or_default());
        self.cfg.set_autosync(CFG_AUTOSYNC_SHALLOW);
        self.cfg.set_reconnect_freq(0);
        self.cfg.set_reconnect_interval("1"); // String, because it can be an expression like "1-5" (random between 1 and 5)

        let cfp = self.cfg.config_path();
        fs::write(&cfp, SysInspectConfig::default().set_minion_config(self.cfg.clone()).to_yaml())?;
        log::info!("📄  Configuration file written to {}", cfp.to_str().unwrap_or_default().bright_white().bold());
        if !self.alt_dir.is_empty() {
            fs::write(self.cfg.local_marker_path(), LocalMarker::hopstart(self.cfg.root_dir().to_str().unwrap_or_default()).to_yaml()?)?;
            log::info!("📌  Local ownership marker written to {}", self.cfg.local_marker_path().display());
        }

        Ok(())
    }

    /// Cleanup everything after the setup
    fn cleanup(&self) -> Result<(), SysinspectError> {
        let self_name = std::env::current_exe().map_err(|e| SysinspectError::ConfigError(format!("Failed to get current executable: {e}")))?;
        self_name
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| SysinspectError::ConfigError("Unable to determine the executable file name".to_string()))?;

        // Remove temporary files in the current directory
        for fname in ["sysinspect.conf", self_name.to_str().unwrap_or_default()] {
            let tmpcfg = PathBuf::from(fname);
            if tmpcfg.exists() {
                fs::remove_file(tmpcfg).map_err(|e| SysinspectError::ConfigError(format!("Failed to remove temporary file: {e}")))?;
            }
        }

        Ok(())
    }

    /// Copy binaries
    fn copy_binaries(&self) -> Result<(), SysinspectError> {
        let bin = std::env::current_exe()?;
        let dst = self.cfg.install_bin_dir().join(bin.file_name().unwrap_or_default());
        fs::copy(&bin, &dst)
            .map_err(|e| SysinspectError::ConfigError(format!("Failed to copy executable to {}: {}", dst.to_str().unwrap_or_default(), e)))?;

        Ok(())
    }

    /// Set alternative directory
    pub fn set_alt_dir(mut self, dir: String) -> Self {
        if dir.is_empty() {
            return self;
        }

        self.alt_dir = dir;
        self.cfg.set_root_dir(&self.alt_dir);
        self.sharelib = format!("{}/share", self.alt_dir);

        self
    }

    /// Set master address
    pub fn set_master_addr(mut self, addr: String) -> Self {
        if addr.is_empty() {
            return self;
        }

        self.master_addr = addr;
        self
    }

    /// Setup the minion
    pub fn setup(&mut self) -> Result<(), SysinspectError> {
        log::info!("⚙️  Setting up the minion. This is a quick setup, please check the configuration files for more details.");

        match Self::check_my_permissions() {
            Ok(_) => {
                log::info!("Permissions OK");
            }
            Err(_) => {
                log::warn!(
                    "🚨  {} is installed as {}, so its operation {}. For full functionality, you have to setup {} as {}.",
                    "SysMinion".bold(),
                    "non-root".bright_red(),
                    "might be limited".bright_yellow(),
                    "SysMinion".bold(),
                    "root".bright_green(),
                );
                log::warn!(
                    "🚨  Don't forget to use \"{}\" to preserve environment variables, when setting up {} as {}.",
                    "sudo -E".yellow(),
                    "SysMinion".bold(),
                    "root".bright_green()
                );
            }
        }

        self.check_dir_structure()?;
        log::info!("📂  Directory structure OK");

        self.generate_dir_structure()?;
        log::info!("📂  Directory structure set up");

        self.generate_config()?;
        log::info!("📄  Configuration file generated to {}", self.cfg.config_dir().display().to_string().bright_white().bold());

        self.copy_binaries()?;
        log::info!("📦  Binaries are copied to {}", self.cfg.install_bin_dir().display().to_string().bright_white().bold());

        self.cleanup()?;
        log::info!("👌  That should do.");

        Ok(())
    }

    /// Set configuration file instance
    pub fn set_config(mut self, cfg: MinionConfig) -> Self {
        self.cfg = cfg;
        self
    }
}

pub fn ensure_minion_tree(cfg: &MinionConfig) -> Result<(), SysinspectError> {
    for dir in [
        cfg.root_dir(),
        cfg.models_dir(),
        cfg.functions_dir(),
        cfg.traits_dir(),
        cfg.sensors_dir(),
        cfg.profiles_dir(),
        cfg.transport_root(),
        cfg.transport_master_root(),
        cfg.pending_tasks_dir(),
    ] {
        if !dir.exists() {
            log::info!("Creating missing directory {}", dir.display());
            fs::create_dir_all(&dir)?;
        } else if !dir.is_dir() {
            return Err(SysinspectError::ConfigError(format!("{} is not a directory", dir.display())));
        }
    }

    Ok(())
}
