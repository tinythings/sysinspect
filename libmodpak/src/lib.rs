use colored::Colorize;
use fs_extra::dir::CopyOptions;
use goblin::{Object, elf::header};
use indexmap::IndexMap;
use libsysinspect::cfg::mmconf::{CFG_AUTOSYNC_FAST, CFG_AUTOSYNC_SHALLOW, DEFAULT_MODULES_LIB_DIR, MinionConfig};
use libsysinspect::{SysinspectError, cfg::mmconf::DEFAULT_MODULES_DIR};
use mpk::{ModAttrs, ModPakMetadata, ModPakRepoIndex};
use std::os::unix::fs::PermissionsExt;
use std::{collections::HashMap, fs, path::PathBuf};

pub mod mpk;

/*
ModPack is a library for creating and managing modules, providing a way to define modules,
their dependencies, and their architecture.
*/

static REPO_MOD_INDEX: &str = "mod.index";

pub struct SysInspectModPakMinion {
    cfg: MinionConfig,
}

impl SysInspectModPakMinion {
    /// Creates a new SysInspectModPakMinion instance.
    pub fn new(cfg: MinionConfig) -> Self {
        Self { cfg }
    }

    async fn get_modpak_idx(&self) -> Result<ModPakRepoIndex, SysinspectError> {
        let resp = reqwest::Client::new()
            .get(format!("http://{}/repo/{}", self.cfg.fileserver(), REPO_MOD_INDEX))
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

    pub fn verify_artefact_by_subpath(
        &self, section: &str, subpath: &str, checksum: &str,
    ) -> Result<(bool, Option<String>), SysinspectError> {
        let path = self.cfg.sharelib_dir().join(section).join(subpath);
        if !path.exists() {
            log::warn!("Required module {} is missing, needs sync", path.display().to_string().bright_yellow());
            return Ok((false, None));
        }

        if self.cfg.autosync().eq(CFG_AUTOSYNC_SHALLOW) {
            log::debug!("Shallow sync: {}", subpath.to_string().bright_yellow());
            return Ok((path.exists(), None));
        }

        let fcs = path.with_extension("checksum.sha256");

        // Shallow-check if the checksum file exists and matches the expected checksum
        if fcs.exists() && self.cfg.autosync().eq(CFG_AUTOSYNC_FAST) {
            log::debug!("Fast sync: {}", subpath.to_string().bright_yellow());
            let buff = fs::read_to_string(fcs)?;
            return Ok((buff.trim() == checksum, Some(buff)));
        }

        log::debug!("Full sync: {}", subpath.to_string().bright_yellow());
        let fcs = libsysinspect::util::iofs::get_file_sha256(path.to_path_buf())?;
        Ok((fcs.eq(checksum), Some(fcs)))
    }

    pub async fn sync(&self) -> Result<(), SysinspectError> {
        match self.cfg.autosync().as_str() {
            v if v == CFG_AUTOSYNC_SHALLOW => log::info!("Shallow data sync with {}", self.cfg.fileserver()),
            v if v == CFG_AUTOSYNC_FAST => log::info!("Fast data sync with {}", self.cfg.fileserver()),
            _ => log::info!("Full data sync with {}", self.cfg.fileserver()),
        }

        let ridx = self.get_modpak_idx().await?;

        self.sync_modules(&ridx).await?;
        self.sync_libraries(&ridx).await?;
        log::info!("Data sync done");
        Ok(())
    }

    /// Syncs libraries from the fileserver.
    async fn sync_libraries(&self, ridx: &ModPakRepoIndex) -> Result<(), SysinspectError> {
        log::info!("Syncing {} library objects", ridx.library().len());
        let libt = ridx.library().len();
        let mut synced = 0;
        log::info!("{}% of library objects synced", (synced * 100) / libt);

        for (_, lf) in ridx.library() {
            let (verified, _) = self
                .verify_artefact_by_subpath(DEFAULT_MODULES_LIB_DIR, lf.file().to_str().unwrap_or_default(), lf.checksum())
                .unwrap_or((false, None));

            if !verified {
                log::info!("Updating library artifact {}", lf.file().display().to_string().bright_yellow());
                let resp = reqwest::Client::new()
                    .get(format!("http://{}/repo/lib/{}", self.cfg.fileserver(), lf.file().display()))
                    .send()
                    .await
                    .map_err(|e| SysinspectError::MasterGeneralError(format!("Request failed: {}", e)))?;
                if resp.status() != reqwest::StatusCode::OK {
                    log::error!("Failed to download library {}: {}", lf.file().display(), resp.status());
                    continue;
                }
                let buff = resp
                    .bytes()
                    .await
                    .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to read response: {}", e)))?;
                let dst = self.cfg.sharelib_dir().join(DEFAULT_MODULES_LIB_DIR).join(lf.file());

                log::debug!("Writing library to {} ({} bytes)", dst.display().to_string().bright_yellow(), buff.len());

                if let Some(dst) = dst.parent() {
                    if !dst.exists() {
                        log::debug!("Creating directory {}", dst.display().to_string().bright_yellow());
                        std::fs::create_dir_all(dst)?;
                    }
                }

                fs::write(&dst, buff)?;
                fs::write(dst.with_extension("checksum.sha256"), lf.checksum())?;
            }

            synced += 1;
            if synced % (libt / 4).max(1) == 0 || synced == libt {
                log::info!("{}% of library objects synced", (synced * 100) / libt);
            }
        }
        log::info!("Syncing libraries from {} done", self.cfg.fileserver());
        Ok(())
    }

    /// Syncs modules from the fileserver.
    async fn sync_modules(&self, ridx: &ModPakRepoIndex) -> Result<(), SysinspectError> {
        let ostype = env!("THIS_OS");
        let osarch = env!("THIS_ARCH");

        let modt = ridx.modules().len();
        log::info!("Syncing {} modules", modt);
        let mut synced = 0;
        log::info!("{}% of modules synced", (synced * 100) / modt);

        // Modules
        for (name, attrs) in ridx.modules() {
            let path = format!(
                "http://{}/repo/{}/{}/{}/{}",
                self.cfg.fileserver(),
                if attrs.mod_type().eq("binary") { "bin" } else { "script" },
                ostype,
                osarch,
                attrs.subpath()
            );
            let dst = self.cfg.sharelib_dir().join(DEFAULT_MODULES_DIR).join(attrs.subpath());
            let dst_cs = dst.with_extension("checksum.sha256");

            let (verified, fsc) =
                self.verify_artefact_by_subpath(DEFAULT_MODULES_DIR, attrs.subpath(), attrs.checksum()).unwrap_or((false, None));
            if !verified {
                log::info!("Downloading module {} from {}", name.bright_yellow(), path);
                let resp = reqwest::Client::new()
                    .get(path)
                    .send()
                    .await
                    .map_err(|e| SysinspectError::MasterGeneralError(format!("Request failed: {}", e)))?;
                if resp.status() != reqwest::StatusCode::OK {
                    log::error!("Failed to download module {}: {}", name, resp.status());
                    continue;
                }
                let buff = resp
                    .bytes()
                    .await
                    .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to read response: {}", e)))?;

                // Check if we need to write that
                log::info!("Writing module to {}", dst.display().to_string().bright_yellow());
                if let Some(pdst) = dst.parent() {
                    if !pdst.exists() {
                        log::debug!("Creating directory {}", pdst.display().to_string().bright_yellow());
                        std::fs::create_dir_all(pdst)?;
                    }
                }
                fs::write(&dst, buff)?;

                // chmod +X
                let mut p = fs::metadata(&dst)?.permissions();
                p.set_mode(0o755);
                fs::set_permissions(&dst, p)?;
            }

            if let Some(fsc) = fsc {
                if !dst_cs.exists() || !verified {
                    log::info!("Updating module checksum as {}", dst_cs.display().to_string().bright_yellow());
                    fs::write(dst_cs, fsc)?;
                }
            }
            synced += 1;
            if synced % (modt / 4).max(1) == 0 || synced == modt {
                log::info!("{}% of modules synced", (synced * 100) / modt);
            }
        }
        log::info!("Syncing modules from {} done", self.cfg.fileserver());
        Ok(())
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
        self.idx.index_library(&path)?;
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
            .index_module(
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
        for (p, archset) in self.idx.all_modules(None) {
            let p = if osn.contains_key(p.as_str()) { osn.get(p.as_str()).unwrap() } else { p.as_str() };
            for (arch, modules) in archset {
                println!("{} ({}): ", p, arch.bright_green());
                Self::print_table(&modules);
            }
        }
        Ok(())
    }
}
