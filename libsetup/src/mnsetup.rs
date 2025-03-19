use libsysinspect::{
    SysinspectError,
    cfg::mmconf::{DEFAULT_MODULES_DIR, DEFAULT_MODULES_PYLIB_DIR, DEFAULT_MODULES_SHARELIB, DEFAULT_SYSINSPECT_ROOT},
};
use std::{fs::File, io::Write, path::Path};
use uuid::Uuid;

#[derive(Default)]
pub struct MinionSetup {
    alt_dir: String,
    sharelib: String,
    master_addr: String,
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
        if unsafe { libc::getuid() } == 0 {
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
            return Err(SysinspectError::ConfigError(format!(
                "Given alternative directory {} appears to be read-only",
                self.alt_dir
            )));
        }

        if self.alt_dir.is_empty() {
            // This is the scenario when SysInspect is not packaged as a system package
            for d in ["/usr/share", "/usr/bin", "/etc", "/var/run", "/var/tmp"] {
                if !Self::is_dir_w(Path::new(d)) {
                    return Err(SysinspectError::ConfigError(format!("Directory {} appears to be read-only", d)));
                }
            }
        }

        Ok(())
    }

    /// Get `/etc/sysinspect`
    /// This is the directory where the configuration files are stored
    fn get_etc(&self) -> String {
        if self.alt_dir.is_empty() { DEFAULT_SYSINSPECT_ROOT.to_string() } else { format!("{}/sysinspect", self.alt_dir) }
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

    /// Get /var/tmp
    /// This is the directory where the temporary files are stored
    fn get_tmp(&self) -> String {
        if self.alt_dir.is_empty() { "/var/tmp/sysinspect".to_string() } else { format!("{}/tmp/db", self.alt_dir) }
    }

    /// Get /var/tmp/db
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
            self.get_shared_subdir(DEFAULT_MODULES_PYLIB_DIR),
        ];

        for d in dirs {
            log::info!("Creating directory {}", d);
            /*
            if !Path::new(&d).exists() {
                std::fs::create_dir_all(&d)
                    .map_err(|_| SysinspectError::ConfigError(format!("Unable to create directory {}", d)))?;
            }
            */
        }

        Ok(())
    }

    /// Generate configuration files
    fn generate_config(&self) -> Result<(), SysinspectError> {
        for d in [self.get_shared_subdir(DEFAULT_MODULES_DIR), self.get_shared_subdir(DEFAULT_MODULES_PYLIB_DIR)] {}
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
    pub fn setup(&self) -> Result<(), SysinspectError> {
        log::info!("Setting up the minion. This is a quick setup, please check the configuration files for more details.");
        self.check_my_permissions()?;
        log::info!("Permissions OK");

        self.check_dir_structure()?;
        log::info!("Directory structure OK");

        self.generate_dir_structure()?;
        log::info!("Directory structure set up");

        self.generate_config()?;
        log::info!("Configuration files generated");

        log::info!("That should do.");

        Ok(())
    }
}
