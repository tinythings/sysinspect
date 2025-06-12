use anyhow::Context;
use colored::Colorize;
use indexmap::IndexMap;
use libmodcore::modinit::ModInterface;
use libsysinspect::SysinspectError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModAttrs {
    subpath: String,
    descr: String,

    #[serde(rename = "type")]
    mod_type: String,

    #[serde(rename = "sha256")]
    checksum: String,
}

impl ModAttrs {
    /// Creates a new ModAttrs with the given subpath, description, and type.
    pub fn new(subpath: String, descr: String, mod_type: String, checksum: String) -> Self {
        Self { subpath, descr, mod_type, checksum }
    }

    /// Returns the subpath of the module.
    pub fn subpath(&self) -> &str {
        &self.subpath
    }

    /// Returns the description of the module.
    pub fn descr(&self) -> &str {
        &self.descr
    }

    /// Returns the type of the module.
    pub fn mod_type(&self) -> &str {
        &self.mod_type
    }

    /// Returns the checksum of the module.
    pub fn checksum(&self) -> &str {
        &self.checksum
    }
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModPakRepoLibFile {
    file: PathBuf,
    checksum: String,
}

impl ModPakRepoLibFile {
    /// Creates a new ModPakRepoLibFile with the given file and checksum.
    pub fn new(file: PathBuf, checksum: &str) -> Self {
        Self { file, checksum: checksum.to_string() }
    }

    /// Returns the file of the library file.
    pub fn file(&self) -> &PathBuf {
        &self.file
    }

    /// Returns the checksum of the library file.
    pub fn checksum(&self) -> &str {
        &self.checksum
    }
}

#[allow(clippy::type_complexity)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ModPakRepoIndex {
    /// Platform -> Architecture -> Module name
    /// e.g. "linux" -> "x86_64" -> "fs.file" -> key/value (name, descr, version etc)
    platform: IndexMap<String, IndexMap<String, IndexMap<String, ModAttrs>>>,

    /// Simply files. They are all the same on all minions for all platforms and architectures.
    /// Usually they are meant to be just Python scripts. Possibly .so files could be also
    /// there, but they have to be unique in naming for each platform/arch and linked
    /// accordingly.
    library: IndexMap<String, ModPakRepoLibFile>,
}

impl Default for ModPakRepoIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ModPakRepoIndex {
    /// Creates a new ModPakRepoIndex.
    pub fn new() -> Self {
        Self { platform: IndexMap::new(), library: IndexMap::new() }
    }

    pub fn index_library(&mut self, p: &Path) -> Result<(), SysinspectError> {
        for (fname, cs) in libsysinspect::util::iofs::scan_files_sha256(p.to_path_buf(), None) {
            log::debug!("Adding library file: {} with checksum: {}", fname, cs);
            self.library.insert(fname.clone(), ModPakRepoLibFile::new(PathBuf::from(fname), &cs));
        }

        Ok(())
    }

    /// Adds a module to the index.
    #[allow(clippy::too_many_arguments)]
    pub fn index_module(
        &mut self, name: &str, subpath: &str, platform: &str, arch: &str, descr: &str, bin: bool, checksum: &str,
    ) -> Result<(), SysinspectError> {
        let attrs = ModAttrs {
            subpath: subpath.to_string(),
            descr: descr.to_string(),
            mod_type: if bin { "binary".to_string() } else { "script".to_string() },
            checksum: checksum.to_string(),
        };

        self.platform.entry(platform.to_string()).or_default().entry(arch.to_string()).or_default().insert(name.to_string(), attrs);

        Ok(())
    }

    /// Deletes a module from the index.
    pub fn remove_module(&mut self, name: &str, platform: &str, arch: &str) -> Result<(), SysinspectError> {
        if let Some(platform_map) = self.platform.get_mut(platform) {
            if let Some(arch_map) = platform_map.get_mut(arch) {
                arch_map.shift_remove(name);
            }
        }

        Ok(())
    }

    /// Deletes all modules with the given name from the index for all platforms and architectures.
    pub fn remove_module_all(&mut self, name: Vec<&str>) -> Result<(), SysinspectError> {
        for n in name {
            for (_, platform_map) in self.platform.iter_mut() {
                for (_, arch_map) in platform_map.iter_mut() {
                    arch_map.shift_remove(n);
                }
            }
        }
        Ok(())
    }

    /// Lists all modules in the index.
    pub fn remove_library(&mut self, name: &str) -> Result<(), SysinspectError> {
        self.library.shift_remove(name);
        Ok(())
    }

    /// Returns scanned library data
    pub fn library(&self) -> IndexMap<String, ModPakRepoLibFile> {
        {
            let mut sorted_library = self.library.clone();
            sorted_library.sort_keys();
            sorted_library
        }
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

    pub(crate) fn modules(&self) -> IndexMap<String, ModAttrs> {
        let mut modules = IndexMap::new();
        let os_types = vec![env!("THIS_OS").to_string(), "any".to_string()];
        let arch_types = vec![env!("THIS_ARCH").to_string(), "noarch".to_string()];

        for ostype in os_types {
            if let Some(platform_map) = self.platform.get(&ostype) {
                for osarch in &arch_types {
                    if let Some(arch_map) = platform_map.get(osarch) {
                        for (name, attrs) in arch_map.iter() {
                            modules.insert(name.clone(), attrs.clone());
                        }
                    } else {
                        log::warn!("No modules for arch: {}", osarch);
                    }
                }
            }
        }
        modules
    }

    /// Returns the modules in the index. Optionally filtered by architecture and names.
    pub(crate) fn all_modules(&self, arch: Option<&str>, names: Option<Vec<&str>>) -> IndexMap<String, IndexMap<String, IndexMap<String, ModAttrs>>> {
        if let Some(arch) = arch {
            self.platform
                .iter()
                .filter_map(|(platform, arch_map)| {
                    if let Some(mod_map) = arch_map.get(arch) {
                        let mut filtered_mod_map = mod_map.clone();

                        if let Some(names) = &names {
                            filtered_mod_map.retain(|name, _| names.contains(&name.as_str()));
                        }

                        if !filtered_mod_map.is_empty() {
                            let mut new_arch_map = IndexMap::new();
                            new_arch_map.insert(arch.to_string(), filtered_mod_map);
                            Some((platform.clone(), new_arch_map))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            let mut filtered_platform = self.platform.clone();
            if let Some(names) = &names {
                for (_, arch_map) in filtered_platform.iter_mut() {
                    for (_, mod_map) in arch_map.iter_mut() {
                        mod_map.retain(|name, _| names.contains(&name.as_str()));
                    }
                }
            }

            filtered_platform
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ModPakMetadata {
    path: PathBuf,
    name: String,
    descr: String,
    arch: String,
}

impl ModPakMetadata {
    pub fn set_arch(&mut self, arch: &str) {
        self.arch = arch.to_string();
    }

    /// Returns the path to the module.
    pub fn get_path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the name of the module.
    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_subpath(&self) -> PathBuf {
        let p = self.get_name().trim_start_matches('.').trim_end_matches('.').to_string().replace('.', "/");
        if self.arch.eq("noarch") { PathBuf::from(format!("{}.py", p)) } else { PathBuf::from(p) }
    }

    pub fn from_cli_matches(matches: &clap::ArgMatches) -> Result<Self, SysinspectError> {
        let mut mpm = ModPakMetadata::default();

        if let Some(path) = matches.get_one::<String>("path") {
            mpm.path = PathBuf::from(path);
        }

        let spec = PathBuf::from(format!("{}.spec", &mpm.path.display()));
        let mi: ModInterface = if spec.exists() {
            serde_yaml::from_str(std::fs::read_to_string(&spec).with_context(|| format!("Unable to read spec file at {}", spec.display()))?.as_str())
                .with_context(|| "Gibberish in spec file?")?
        } else {
            ModInterface::default()
        };

        if let Some(name) = matches.get_one::<String>("name") {
            mpm.name = name.clone();
        } else if !mi.name().is_empty() {
            mpm.name = mi.name().to_string();
        }
        if mpm.name.is_empty() {
            return Err(SysinspectError::InvalidModuleName(
                format!("name was not obtained. Either add a spec file or use the {} option.", "--name".bright_yellow()).to_string(),
            ));
        }

        if let Some(descr) = matches.get_one::<String>("descr") {
            mpm.descr = descr.clone();
        } else if !mi.description().is_empty() {
            mpm.descr = mi.description().to_string().replace("\n", "");
        }
        if mpm.descr.is_empty() {
            return Err(SysinspectError::InvalidModuleName(
                format!("description was not obtained. Either add a spec file or use the {} option.", "--descr".bright_yellow()).to_string(),
            ));
        }

        log::info!("Adding module at {}", mpm.path.display());

        Ok(mpm)
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

    /// Returns true if the module is a binary module.
    pub fn is_binary(&self) -> bool {
        self.binary
    }
}
