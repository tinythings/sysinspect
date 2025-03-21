use libsysinspect::SysinspectError;
use mpk::{ModPackModule, ModPakArch};
use std::{fs, path::PathBuf};

pub mod mpk;

/*
ModPack is a library for creating and managing modules, providing a way to define modules,
their dependencies, and their architecture.
*/

/// ModPakRepo is a repository for storing and managing modules.
pub struct SysInspectModPak {
    root: PathBuf,
}

impl SysInspectModPak {
    /// Creates a new ModPakRepo with the given root path.
    pub fn new(root: PathBuf) -> Result<Self, SysinspectError> {
        if !root.exists() {
            log::info!("Creating module repository at {}", root.display());
            std::fs::create_dir_all(&root)?;
        }

        Ok(Self { root })
    }

    /// Add an existing binary module.
    pub fn add_module(&self, p: ModPackModule) -> Result<(), SysinspectError> {
        let path = self.root.join(p.get_subpath());
        log::info!("Adding module {}", path.display());
        Ok(())
    }

    pub fn list_modules(&self) -> Result<Vec<ModPackModule>, SysinspectError> {
        let mut modules = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(arch) = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) {
                        modules.push(ModPackModule::new(name.to_string(), ModPakArch::Noarch, false)?);
                    }
                }
            }
        }
        Ok(modules)
    }

    pub fn remove_module(&self, name: &str, arch: Option<ModPakArch>) -> Result<(), SysinspectError> {
        let path = fs::canonicalize(self.get_module(name, arch)?)?;
        if path.exists() {
            std::fs::remove_dir_all(path)?;
        }
        Ok(())
    }

    /// Get module location.
    /// If the module is a binary module, it will return the path to the binary.
    pub fn get_module(&self, name: &str, arch: Option<ModPakArch>) -> Result<String, SysinspectError> {
        Ok("".to_string())
    }
}
