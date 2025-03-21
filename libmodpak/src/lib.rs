use colored::Colorize;
use goblin::{Object, elf::header};
use indexmap::IndexMap;
use libsysinspect::SysinspectError;
use mpk::{ModPakMetadata, ModPakRepoIndex};
use std::{collections::HashMap, fs, path::PathBuf};

pub mod mpk;

/*
ModPack is a library for creating and managing modules, providing a way to define modules,
their dependencies, and their architecture.
*/

static REPO_INDEX: &str = "repo.index";
/// ModPakRepo is a repository for storing and managing modules.
pub struct SysInspectModPak {
    root: PathBuf,
    idx: ModPakRepoIndex,
}

impl SysInspectModPak {
    /// Creates a new ModPakRepo with the given root path.
    pub fn new(root: PathBuf) -> Result<Self, SysinspectError> {
        if !root.exists() {
            log::info!("Creating module repository at {}", root.display());
            std::fs::create_dir_all(&root)?;
            fs::write(root.join(REPO_INDEX), ModPakRepoIndex::new().to_yaml()?)?;
        }

        Ok(Self { root: root.clone(), idx: ModPakRepoIndex::from_yaml(&fs::read_to_string(root.join(REPO_INDEX))?)? })
    }

    /// Get osabi label
    fn get_osabi_label(osabi: u8) -> &'static str {
        match osabi {
            header::ELFOSABI_SYSV => "sysv",
            header::ELFOSABI_NETBSD => "netbsd",
            header::ELFOSABI_LINUX => "linux",
            header::ELFOSABI_FREEBSD => "freebsd",
            header::ELFOSABI_OPENBSD => "openbsd",
            header::ELFOSABI_ARM => "arm",
            header::ELFOSABI_ARM_AEABI => "arm-eabi",
            header::ELFOSABI_STANDALONE => "standalone",
            _ => "any",
        }
    }

    /// Extract module subpath from its name
    fn after<'a>(full_path: &'a str, sub: &'a str) -> &'a str {
        if let Some(index) = full_path.find(sub) {
            let result = &full_path[index..];
            if result.len() == sub.len() { sub } else { result }
        } else {
            sub
        }
    }

    /// Add an existing binary module.
    pub fn add_module(&mut self, meta: ModPakMetadata) -> Result<(), SysinspectError> {
        let path = fs::canonicalize(meta.get_path())?;
        log::info!("Adding module {}", path.display().to_string().bright_yellow());

        let buff = fs::read(path)?;
        let (is_bin, arch, p) = match Object::parse(&buff).unwrap() {
            Object::Elf(elf) => {
                let x = match elf.header.e_machine {
                    goblin::elf::header::EM_X86_64 => {
                        log::info!("Architecture: x86_64 ELF");
                        "x86_64"
                    }
                    goblin::elf::header::EM_ARM => {
                        log::info!("Architecture: ARM ELF");
                        "ARM"
                    }
                    goblin::elf::header::EM_AARCH64 => {
                        log::info!("Architecture: ARM64 ELF");
                        "ARM64"
                    }
                    _ => {
                        return Err(SysinspectError::MasterGeneralError(
                            "Module is not a supported ELF architecture".to_string(),
                        ));
                    }
                };
                (true, x, Self::get_osabi_label(elf.header.e_ident[header::EI_OSABI]))
            }
            _ => {
                log::info!("Module is not an executable ELF file");
                (false, "noarch", "any")
            }
        };

        log::info!("Platform: {}", p);
        let module_subpath = PathBuf::from(Self::after(
            meta.get_path().to_str().unwrap_or_default(),
            meta.get_subpath().to_str().unwrap_or_default(),
        ));
        let subpath = PathBuf::from(format!("{}/{}/{}", if is_bin { "bin" } else { "script" }, p, arch)).join(&module_subpath);
        log::debug!("Subpath: {}", subpath.display().to_string().bright_yellow());
        if let Some(p) = self.root.join(&subpath).parent() {
            if !p.exists() {
                log::debug!("Creating directory {}", p.display().to_string().bright_yellow());
                std::fs::create_dir_all(p).unwrap();
            }
        }
        log::debug!("Copying module to {}", self.root.join(&subpath).display().to_string().bright_yellow());
        std::fs::copy(meta.get_path(), self.root.join(&subpath))?;

        self.idx
            .add_module(meta.get_name().as_str(), module_subpath.to_str().unwrap_or_default(), p, arch, meta.get_descr())
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to add module to index: {}", e)))?;
        log::debug!("Writing index to {}", self.root.join(REPO_INDEX).display().to_string().bright_yellow());
        fs::write(self.root.join(REPO_INDEX), self.idx.to_yaml()?)?;
        log::debug!("Module {} added to index", meta.get_name().bright_yellow());
        log::info!("Module {} added successfully", meta.get_name().bright_yellow());

        Ok(())
    }

    fn print_table(modules: &IndexMap<String, IndexMap<String, String>>) {
        let c_width = modules.keys().map(|s| s.len()).max().unwrap_or(0);
        let mk_width = modules.values().flat_map(|data| data.keys()).map(|k| k.len()).max().unwrap_or(0);
        let mut modules: Vec<_> = modules.iter().collect();
        modules.sort_by_key(|(name, _)| name.as_str());

        for (name, attr) in modules {
            let mut attr: Vec<(&String, &String)> = attr.iter().filter(|(k, _)| *k != "subpath").collect();
            attr.sort_by_key(|(key, _)| key.as_str());

            if let Some((k, v)) = attr.first() {
                println!(
                    "    {:<mod_width$}  {k:>key_width$}: {v}",
                    name.bright_white().bold(),
                    mod_width = c_width,
                    k = k.yellow(),
                    key_width = mk_width,
                    v = v
                );

                for (k, v) in attr.iter().skip(1) {
                    println!(
                        "    {:<mod_width$}  {k:>key_width$}: {v}",
                        "",
                        mod_width = c_width,
                        k = k.yellow(),
                        key_width = mk_width,
                        v = v
                    );
                }
            } else {
                println!("    {:<mod_width$}", name, mod_width = c_width);
            }
            println!()
        }
    }

    pub fn list_modules(&self) -> Result<(), SysinspectError> {
        let osn = HashMap::from([("sysv", "Linux"), ("any", "Any")]);
        for (p, archset) in self.idx.get_modules(None) {
            let p = if osn.contains_key(p.as_str()) { osn.get(p.as_str()).unwrap() } else { p.as_str() };
            for (arch, modules) in archset {
                println!("{} ({}): ", p, arch.bright_green());
                Self::print_table(&modules);
            }
        }
        Ok(())
    }

    pub fn remove_module(&self, name: &str) -> Result<(), SysinspectError> {
        Ok(())
    }

    /// Get module location.
    /// If the module is a binary module, it will return the path to the binary.
    pub fn get_module(&self, name: &str) -> Result<String, SysinspectError> {
        Ok("".to_string())
    }
}
