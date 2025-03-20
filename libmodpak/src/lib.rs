use libsysinspect::SysinspectError;
use mpk::{ModPackModule, ModPakArch};
use std::path::PathBuf;

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
            std::fs::create_dir_all(&root)?;
        }

        Ok(Self { root })
    }

    /// Add an existing binary module.
    pub fn add_bin_module(&self, p: ModPackModule) -> Result<(), SysinspectError> {
        Ok(())
    }

    /// Get module location.
    /// If the module is a binary module, it will return the path to the binary.
    pub fn get_bin_module(&self, name: &str, arch: Option<ModPakArch>) -> Result<String, SysinspectError> {
        Ok("".to_string())
    }

    pub fn remove_bin_module(&self, name: &str) -> Result<(), SysinspectError> {
        Ok(())
    }
}
