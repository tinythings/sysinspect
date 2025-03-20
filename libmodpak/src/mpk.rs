use std::path::{Path, PathBuf};

use libsysinspect::SysinspectError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum ModPakArch {
    X86,
    X64,
    ARM,
    ARM64,
    Noarch,
}

/// Module is a single unit of functionality that can be used in a ModPack.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModPackModule {
    arch: ModPakArch,
    name: String, // Module name as in model, e.g. "fs.file"
    binary: bool,
}

impl ModPackModule {
    /// Creates a new ModPackModule with the given name and architecture.
    pub fn new(name: String, arch: ModPakArch, binary: bool) -> Result<Self, SysinspectError> {
        if !name.contains(".") {
            return Err(SysinspectError::InvalidModuleName(format!("Module \"{}\" must have a namespace", name)));
        }

        Ok(Self { name: name.trim_start_matches('.').trim_end_matches('.').to_string(), arch, binary })
    }

    fn get_name_subpath(&self) -> String {
        self.name.clone().replace('.', "/")
    }

    /// Returns architecture of the module.
    fn get_arch_label(&self) -> String {
        match self.arch {
            ModPakArch::X86 => "x86".to_string(),
            ModPakArch::X64 => "x64".to_string(),
            ModPakArch::ARM => "arm".to_string(),
            ModPakArch::ARM64 => "arm64".to_string(),
            ModPakArch::Noarch => "noarch".to_string(),
        }
    }

    /// Get subpath to the module
    pub fn get_subpath(&self) -> PathBuf {
        Path::new("modules").join(self.get_arch_label()).join(&self.name)
    }

    /// Returns true if the module is a binary module.
    pub fn is_binary(&self) -> bool {
        self.binary
    }
}
