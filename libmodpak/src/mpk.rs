use indexmap::IndexMap;
use libsysinspect::SysinspectError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[allow(clippy::type_complexity)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ModPakRepoIndex {
    /// Platform -> Architecture -> Module name
    /// e.g. "linux" -> "x86_64" -> "fs.file" -> key/value (name, descr, version etc)
    platform: IndexMap<String, IndexMap<String, IndexMap<String, IndexMap<String, String>>>>,
}

impl Default for ModPakRepoIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ModPakRepoIndex {
    /// Creates a new ModPakRepoIndex.
    pub fn new() -> Self {
        Self { platform: IndexMap::new() }
    }

    /// Adds a module to the index.
    pub fn add_module(
        &mut self, name: &str, subpath: &str, platform: &str, arch: &str, descr: &str,
    ) -> Result<(), SysinspectError> {
        // XXX: the method should have rather a struct as options instead of 42 parameters :-(
        let module = self
            .platform
            .entry(platform.to_string())
            .or_default()
            .entry(arch.to_string())
            .or_default()
            .entry(name.to_string())
            .or_default();

        module.insert("subpath".to_string(), subpath.to_string());
        module.insert("descr".to_string(), descr.to_string());

        Ok(())
    }

    /// Deletes a module from the index.
    pub fn del_module(&mut self, name: &str, platform: &str, arch: &str) -> Result<(), SysinspectError> {
        if let Some(platform_map) = self.platform.get_mut(platform) {
            if let Some(arch_map) = platform_map.get_mut(arch) {
                arch_map.shift_remove(name);
            }
        }

        Ok(())
    }

    /// Serializes the index to a YAML string.
    pub fn to_yaml(&self) -> Result<String, SysinspectError> {
        let yaml = serde_yaml::to_string(self)?;
        Ok(yaml)
    }

    /// Deserializes a YAML string to a ModPakRepoIndex.
    pub fn from_yaml(yaml: &str) -> Result<Self, SysinspectError> {
        let index: ModPakRepoIndex = serde_yaml::from_str(yaml)?;
        Ok(index)
    }

    #[allow(clippy::type_complexity)]
    /// Returns the modules in the index. Optionally filtered by architecture.
    pub(crate) fn get_modules(
        &self, arch: Option<&str>,
    ) -> IndexMap<String, IndexMap<String, IndexMap<String, IndexMap<String, String>>>> {
        if let Some(arch) = arch {
            self.platform
                .iter()
                .filter(|(_, arch_map)| arch_map.contains_key(arch))
                .map(|(platform, arch_map)| (platform.clone(), arch_map.clone()))
                .collect()
        } else {
            self.platform.clone()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ModPakMetadata {
    path: PathBuf,
    name: String,
    descr: String,
}

impl ModPakMetadata {
    /// Returns the path to the module.
    pub fn get_path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the name of the module.
    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_subpath(&self) -> PathBuf {
        self.get_name().trim_start_matches('.').trim_end_matches('.').to_string().replace('.', "/").into()
    }

    pub fn from_cli_matches(matches: &clap::ArgMatches) -> Self {
        let mut mpm = ModPakMetadata::default();

        if let Some(path) = matches.get_one::<String>("path") {
            mpm.path = PathBuf::from(path);
        }

        if let Some(name) = matches.get_one::<String>("name") {
            mpm.name = name.clone();
        }

        if let Some(descr) = matches.get_one::<String>("descr") {
            mpm.descr = descr.clone();
        }

        mpm
    }

    pub(crate) fn get_descr(&self) -> &str {
        &self.descr
    }
}

/// Module is a single unit of functionality that can be used in a ModPack.
#[derive(Debug, Serialize, Deserialize)]
pub struct ModPackModule {
    name: String, // Module name as in model, e.g. "fs.file"
    binary: bool,
}

impl ModPackModule {
    /// Creates a new ModPackModule with the given name and architecture.
    pub fn new(name: String, binary: bool) -> Result<Self, SysinspectError> {
        if !name.contains(".") {
            return Err(SysinspectError::InvalidModuleName(format!("Module \"{}\" must have a namespace", name)));
        }

        Ok(Self { name: name.trim_start_matches('.').trim_end_matches('.').to_string(), binary })
    }

    fn get_name_subpath(&self) -> String {
        self.name.clone().replace('.', "/")
    }
    /// Returns true if the module is a binary module.
    pub fn is_binary(&self) -> bool {
        self.binary
    }
}
