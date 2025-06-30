use libsysinspect::{
    SysinspectError,
    cfg::mmconf::{
        CFG_AUTOSYNC_SHALLOW, DEFAULT_MODULES_DIR, DEFAULT_MODULES_LIB_DIR, DEFAULT_MODULES_SHARELIB, DEFAULT_SYSINSPECT_ROOT, MinionConfig,
        SysInspectConfig,
    },
};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

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

    fn check_my_permissions(&self) -> Result<(), SysinspectError> {
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
            for d in ["/usr/share", "/usr/bin", "/etc", "/var/run", "/var/tmp"] {
                if !Self::is_dir_w(Path::new(d)) {
                    return Err(SysinspectError::ConfigError(format!("Directory {d} appears to be read-only")));
                }
            }
        }

        Ok(())
    }

    /// Get `/etc/sysinspect`
    /// This is the directory where the configuration files are stored
    fn get_etc(&self) -> String {
        if self.alt_dir.is_empty() { DEFAULT_SYSINSPECT_ROOT.to_string() } else { format!("{}/etc", self.alt_dir) }
    }

    /// Get /usr/bin
    /// This is the directory where the binaries are stored
    fn get_bin(&self) -> String {
        if self.alt_dir.is_empty() { "/usr/bin".to_string() } else { format!("{}/bin", self.alt_dir) }
    }

    /// Get /var/run
    /// This is the directory where the runtime files are stored
    fn get_run(&self) -> String {
        if self.alt_dir.is_empty() { "/var/run".to_string() } else { format!("{}/run", self.alt_dir) }
    }

    /// Get /var/tmp/sysinspect
    /// This is the directory where the temporary files are stored
    fn get_tmp(&self) -> String {
        if self.alt_dir.is_empty() { "/var/tmp/sysinspect".to_string() } else { format!("{}/tmp/db", self.alt_dir) }
    }

    /// Get /tmp
    /// This is the directory where the database files are stored
    fn get_db(&self) -> String {
        if self.alt_dir.is_empty() { "/tmp".to_string() } else { format!("{}/tmp", self.alt_dir) }
    }

    /// Generate directory structure
    fn generate_dir_structure(&self) -> Result<(), SysinspectError> {
        let dirs = [
            self.get_etc(),
            self.get_bin(),
            self.get_run(),
            self.get_tmp(),
            self.get_db(),
            self.get_shared_subdir(DEFAULT_MODULES_DIR),
            self.get_shared_subdir(DEFAULT_MODULES_LIB_DIR),
        ];

        for d in dirs {
            if !Path::new(&d).exists() {
                std::fs::create_dir_all(&d).map_err(|_| SysinspectError::ConfigError(format!("Unable to create directory {d}")))?;
            }
        }

        Ok(())
    }

    /// Generate configuration files
    fn generate_config(&mut self) -> Result<(), SysinspectError> {
        #[allow(clippy::unnecessary_to_owned)]
        self.cfg.set_sharelib_path(&self.get_sharelib().to_string());
        self.cfg.set_pid_path(PathBuf::from(self.get_run()).join("sysinspect.pid").to_str().unwrap_or_default());
        self.cfg.set_autosync(CFG_AUTOSYNC_SHALLOW);
        self.cfg.set_reconnect_freq(0);
        self.cfg.set_reconnect_interval("1"); // String, because it can be an expression like "1-5" (random between 1 and 5)

        let cfp = PathBuf::from(self.get_etc()).join("sysinspect.conf");
        log::info!("Writing configuration file to {}", cfp.to_str().unwrap_or_default());

        fs::write(cfp, SysInspectConfig::default().set_minion_config(self.cfg.clone()).to_yaml())?;

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
        let dst = PathBuf::from(self.get_bin()).join(bin.file_name().unwrap_or_default());
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
        log::info!("Setting up the minion. This is a quick setup, please check the configuration files for more details.");

        self.check_my_permissions()?;
        log::info!("Permissions OK");

        self.check_dir_structure()?;
        log::info!("Directory structure OK");

        self.generate_dir_structure()?;
        log::info!("Directory structure set up");

        self.generate_config()?;
        log::info!("Configuration file generated to {}", self.get_etc());

        self.copy_binaries()?;
        log::info!("Binaries are copied to {}", self.get_bin());

        self.cleanup()?;
        log::info!("That should do.");

        Ok(())
    }

    /// Set configuration file instance
    pub fn set_config(mut self, cfg: MinionConfig) -> Self {
        self.cfg = cfg;
        self
    }
}
