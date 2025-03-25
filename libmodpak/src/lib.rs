use colored::Colorize;
use fs_extra::dir::CopyOptions;
use goblin::{Object, elf::header};
use indexmap::IndexMap;
use libsysinspect::{SysinspectError, cfg::mmconf::MinionConfig};
use mpk::{ModAttrs, ModPakMetadata, ModPakRepoIndex};
use std::{collections::HashMap, fs, path::PathBuf};

pub mod mpk;

/*
ModPack is a library for creating and managing modules, providing a way to define modules,
their dependencies, and their architecture.
*/

static REPO_MOD_INDEX: &str = "mod.index";

pub struct SysInspectModPakMinion {
    url: String,
}

impl SysInspectModPakMinion {
    /// Creates a new SysInspectModPakMinion instance.
    pub fn new(cfg: MinionConfig) -> Self {
        Self { url: cfg.fileserver() }
    }

    async fn get_modpak_idx(&self) -> Result<ModPakRepoIndex, SysinspectError> {
        let resp = reqwest::Client::new()
            .get(format!("http://{}/repo/{}", self.url.clone(), REPO_MOD_INDEX))
            .send()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Request failed: {}", e)))?;
        if resp.status() != reqwest::StatusCode::OK {
            return Err(SysinspectError::MasterGeneralError(format!("Failed to get modpak index: {}", resp.status())));
        }

        let buff = resp.bytes().await.unwrap();
        let idx = ModPakRepoIndex::from_yaml(&String::from_utf8_lossy(&buff))?;
        Ok(idx)
    }

    pub async fn sync_modules(&self) {
        let ostype = env!("THIS_OS");
        let osarch = env!("THIS_ARCH");

        log::info!("Syncing modules from {}", self.url);
        let ridx = self.get_modpak_idx().await.unwrap();
        for (name, attrs) in ridx.get_modules() {
            let path = format!(
                "http://{}/repo/{}/{}/{}/{}",
                self.url,
                if attrs.mod_type().eq("binary") { "bin" } else { "script" },
                ostype,
                osarch,
                attrs.subpath()
            );
            log::info!("Downloading module {} from {}", name.bright_yellow(), path);
            let resp = reqwest::Client::new()
                .get(path)
                .send()
                .await
                .map_err(|e| SysinspectError::MasterGeneralError(format!("Request failed: {}", e)))
                .unwrap();
            if resp.status() != reqwest::StatusCode::OK {
                log::error!("Failed to download module {}: {}", name, resp.status());
                continue;
            }
            let buff = resp.bytes().await.unwrap();
            println!("Got byte length: {}", buff.len());
            //fs::write(attrs.get_path(), buff).unwrap();
        }
        log::info!("Syncing modules from {} done", self.url);
    }

    /// Get module location.
    /// If the module is a binary module, it will return the path to the binary.
    /// This is also works for the current platform and architecture.
    pub fn get_module(&self, name: &str) -> Result<String, SysinspectError> {
        Ok("".to_string())
    }
}

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
            fs::write(root.join(REPO_MOD_INDEX), ModPakRepoIndex::new().to_yaml()?)?;
        }

        let ridx = root.join(REPO_MOD_INDEX);
        if !ridx.exists() {
            log::info!("Creating module repository index at {}", ridx.display());
            fs::write(&ridx, ModPakRepoIndex::new().to_yaml()?)?;
        }

        Ok(Self { root: root.clone(), idx: ModPakRepoIndex::from_yaml(&fs::read_to_string(ridx)?)? })
    }

    /// Get osabi label
    fn get_osabi_label(osabi: u8) -> &'static str {
        match osabi {
            header::ELFOSABI_SYSV | header::ELFOSABI_LINUX => "linux",
            header::ELFOSABI_NETBSD => "netbsd",
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

    pub fn add_library(&mut self, p: PathBuf) -> Result<(), SysinspectError> {
        let path = self.root.join("lib");
        if !path.exists() {
            log::info!("Creating module repository at {}", path.display());
            std::fs::create_dir_all(&path)?;
        }

        let mut options = CopyOptions::new();
        options.overwrite = true; // Overwrite existing files if necessary
        options.copy_inside = true; // Copy the contents inside `p` instead of the directory itself

        log::info!("Copying library from {} to {}", p.display(), path.display());
        fs_extra::dir::copy(&p, &path, &options)
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to copy library: {}", e)))?;
        self.idx.add_library(&path)?;
        log::debug!("Writing index to {}", self.root.join(REPO_MOD_INDEX).display().to_string().bright_yellow());
        fs::write(self.root.join(REPO_MOD_INDEX), self.idx.to_yaml()?)?;
        log::info!("Library {} added to index", p.display().to_string().bright_yellow());
        Ok(())
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
        let checksum = libsysinspect::util::iofs::get_file_sha256(self.root.join(&subpath))?;

        self.idx
            .add_module(
                meta.get_name().as_str(),
                module_subpath.to_str().unwrap_or_default(),
                p,
                arch,
                meta.get_descr(),
                subpath.to_str().unwrap_or_default().starts_with("bin/"),
                &checksum,
            )
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to add module to index: {}", e)))?;
        log::debug!("Writing index to {}", self.root.join(REPO_MOD_INDEX).display().to_string().bright_yellow());
        fs::write(self.root.join(REPO_MOD_INDEX), self.idx.to_yaml()?)?;
        log::debug!("Module {} added to index", meta.get_name().bright_yellow());
        log::info!("Module {} added successfully", meta.get_name().bright_yellow());

        Ok(())
    }

    fn print_table(modules: &IndexMap<String, ModAttrs>) {
        let mw = modules.keys().map(|s| s.len()).max().unwrap_or(0);
        let kw = "descr".len().max("type".len());
        let mut mods: Vec<_> = modules.iter().collect();
        mods.sort_by_key(|(name, _)| *name);

        for (mname, attrs) in mods {
            let mut attrs = [("descr", attrs.descr()), ("type", attrs.mod_type())];
            attrs.sort_by_key(|(k, _)| *k);
            if let Some((first_key, first_value)) = attrs.first() {
                println!(
                    "    {:<mw$}  {:>kw$}: {}",
                    mname.bright_white().bold(),
                    first_key.yellow(),
                    first_value,
                    mw = mw,
                    kw = kw,
                );
                for (k, v) in attrs.iter().skip(1) {
                    println!("    {:<mw$}  {:>kw$}: {}", "", k.yellow(), v, mw = mw, kw = kw,);
                }
            } else {
                println!("    {:<mw$}", mname, mw = mw);
            }
            println!();
        }
    }

    pub fn list_modules(&self) -> Result<(), SysinspectError> {
        let osn = HashMap::from([("sysv", "Linux"), ("any", "Any")]);
        for (p, archset) in self.idx.get_all_modules(None) {
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
}
