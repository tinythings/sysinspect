use colored::Colorize;
use cruet::Inflector;
use fs_extra::dir::CopyOptions;
use goblin::Object;
use indexmap::IndexMap;
use libcommon::SysinspectError;
use libsysinspect::cfg::mmconf::DEFAULT_MODULES_DIR;
use libsysinspect::cfg::mmconf::{CFG_AUTOSYNC_FAST, CFG_AUTOSYNC_SHALLOW, DEFAULT_MODULES_LIB_DIR, MinionConfig};
use libsysinspect::util::iofs::get_file_sha256;
use mpk::{ModAttrs, ModPakMetadata, ModPakRepoIndex};
use once_cell::sync::Lazy;
use prettytable::{Cell, Row, Table, format};
use regex::Regex;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::{collections::HashMap, fs, path::PathBuf};
use textwrap::{Options, wrap};
use tokio::sync::Mutex;

pub mod mpk;

#[cfg(test)]
mod lib_ut;
#[cfg(test)]
mod mpk_ut;

/*
ModPack is a library for creating and managing modules, providing a way to define modules,
their dependencies, and their architecture.
*/

static REPO_MOD_INDEX: &str = "mod.index";
static REPO_MOD_SHA256_EXT: &str = "checksum.sha256";
static ANSI_ESCAPE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*m").expect("ansi regex should compile"));

pub struct ModPakSyncState {
    state: Arc<Mutex<bool>>,
}

impl Default for ModPakSyncState {
    fn default() -> Self {
        Self::new()
    }
}

impl ModPakSyncState {
    pub fn new() -> Self {
        Self { state: Arc::new(Mutex::new(false)) }
    }

    pub async fn set_syncing(&self, sync: bool) {
        let mut state = self.state.lock().await;
        *state = sync;
    }

    pub async fn is_syncing(&self) -> bool {
        let state = self.state.lock().await;
        *state
    }
}

pub static MODPAK_SYNC_STATE: Lazy<ModPakSyncState> = Lazy::new(ModPakSyncState::new);

pub struct SysInspectModPakMinion {
    cfg: MinionConfig,
}

impl SysInspectModPakMinion {
    /// Creates a new SysInspectModPakMinion instance.
    pub fn new(cfg: MinionConfig) -> Self {
        Self { cfg }
    }

    /// Gets the module repository index from the fileserver.
    async fn get_modpak_idx(&self) -> Result<ModPakRepoIndex, SysinspectError> {
        let resp = reqwest::Client::new()
            .get(format!("http://{}/repo/{}", self.cfg.fileserver(), REPO_MOD_INDEX))
            .send()
            .await
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Request failed: {e}")))?;
        if resp.status() != reqwest::StatusCode::OK {
            return Err(SysinspectError::MasterGeneralError(format!("Failed to get modpak index: {}", resp.status())));
        }

        let buff = resp.bytes().await.unwrap();
        let idx = ModPakRepoIndex::from_yaml(&String::from_utf8_lossy(&buff))?;
        Ok(idx)
    }

    /// Verifies an artefact by its subpath and checksum.
    async fn verify_artefact_by_subpath(&self, section: &str, subpath: &str, checksum: &str) -> Result<(bool, Option<String>), SysinspectError> {
        let path = self.cfg.sharelib_dir().join(section).join(subpath);
        if !path.exists() {
            log::debug!("Required module {} is missing, needs sync", path.display().to_string().bright_yellow());
            return Ok((false, None));
        }

        let fcs = path.with_extension(REPO_MOD_SHA256_EXT);
        if fcs.exists() && self.cfg.autosync().eq(CFG_AUTOSYNC_SHALLOW) {
            log::debug!("Shallow check: {}", subpath.to_string().bright_yellow());
            return Ok((path.exists(), None));
        }

        // Shallow-check if the checksum file exists and matches the expected checksum
        if fcs.exists() && self.cfg.autosync().eq(CFG_AUTOSYNC_FAST) {
            log::debug!("Fast check: {}", subpath.to_string().bright_yellow());
            let buff = fs::read_to_string(fcs)?;
            return Ok((buff.trim() == checksum, Some(buff)));
        }

        log::debug!("Full check: {}", subpath.to_string().bright_yellow());
        let fcs = get_file_sha256(path.to_path_buf())?;
        Ok((fcs.eq(checksum), Some(fcs)))
    }

    /// Syncs the module repository with the fileserver.
    pub async fn sync(&self) -> Result<(), SysinspectError> {
        match self.cfg.autosync().as_str() {
            v if v == CFG_AUTOSYNC_SHALLOW => log::info!("Shallow data check with {}", self.cfg.fileserver()),
            v if v == CFG_AUTOSYNC_FAST => log::info!("Fast data check with {}", self.cfg.fileserver()),
            _ => log::info!("Full data check with {}", self.cfg.fileserver()),
        }

        MODPAK_SYNC_STATE.set_syncing(true).await;
        let ridx = self.get_modpak_idx().await?;

        self.sync_integrity(&ridx)?; // blocking
        self.sync_modules(&ridx).await?;
        self.sync_libraries(&ridx).await?;

        MODPAK_SYNC_STATE.set_syncing(false).await;
        log::info!("Data sync done");
        Ok(())
    }

    /// Gets all shared files in the module repository.
    fn get_all_shared(&self) -> Result<IndexMap<String, PathBuf>, SysinspectError> {
        fn collect_files(root: &PathBuf, dir: &PathBuf, shared: &mut IndexMap<String, PathBuf>) -> Result<(), SysinspectError> {
            for e in fs::read_dir(dir).map_err(|e| {
                log::error!("Failed to read directory: {e}");
                SysinspectError::MasterGeneralError(format!("Failed to read directory: {e}"))
            })? {
                let e = e.map_err(|e| {
                    log::error!("Failed to read entry: {e}");
                    SysinspectError::MasterGeneralError(format!("Failed to read entry: {e}"))
                })?;
                let p = e.path();
                if p.is_file() {
                    shared.insert(
                        p.strip_prefix(root)
                            .map_err(|e| SysinspectError::MasterGeneralError(format!("Strip prefix error: {e}")))?
                            .to_string_lossy()
                            .to_string(),
                        p,
                    );
                } else if p.is_dir() {
                    collect_files(root, &p, shared)?;
                }
            }
            Ok(())
        }

        let mut shared = IndexMap::new();
        for sp in [DEFAULT_MODULES_DIR, DEFAULT_MODULES_LIB_DIR].iter() {
            let root = self.cfg.sharelib_dir().join(sp);
            collect_files(&root, &root, &mut shared)?;
        }

        Ok(shared)
    }

    /// Syncs the integrity of the repository by removing any unknown files or those that are no longer in the index.
    fn sync_integrity(&self, ridx: &ModPakRepoIndex) -> Result<(), SysinspectError> {
        log::info!("Checking module integrity");
        let mut unknown = self.get_all_shared()?;

        for attrs in ridx.modules().values() {
            unknown.swap_remove(attrs.subpath());
            unknown.swap_remove(&PathBuf::from(attrs.subpath()).with_extension(REPO_MOD_SHA256_EXT).to_string_lossy().to_string());
        }

        for rfile in ridx.library().values() {
            unknown.swap_remove(rfile.file().to_str().unwrap_or_default());
            unknown.swap_remove(&rfile.file().with_extension(REPO_MOD_SHA256_EXT).to_string_lossy().to_string());
        }

        if unknown.is_empty() {
            log::info!("Integrity check finished");
        } else {
            let mut c = 0;
            for (k, v) in unknown.iter() {
                log::debug!("Removing: {}", k.bright_red());
                fs::remove_file(v)?;
                c += 1;
            }
            log::info!("Integrity check finished with {} orphans removed", c.to_string().bright_red().bold());
        }
        Ok(())
    }

    /// Syncs libraries from the fileserver.
    async fn sync_libraries(&self, ridx: &ModPakRepoIndex) -> Result<(), SysinspectError> {
        log::info!("Syncing {} library objects", ridx.library().len());
        let libt = ridx.library().len();
        let mut synced = 0;

        if libt > 0 {
            log::warn!("{}% of library objects synced", (synced * 100) / libt);
        }

        for (_, lf) in ridx.library() {
            let (verified, _) = self
                .verify_artefact_by_subpath(DEFAULT_MODULES_LIB_DIR, lf.file().to_str().unwrap_or_default(), lf.checksum())
                .await
                .unwrap_or((false, None));

            if !verified {
                log::debug!("Updating library artifact {}", lf.file().display().to_string().bright_yellow());
                let resp = reqwest::Client::new()
                    .get(format!("http://{}/repo/lib/{}", self.cfg.fileserver(), lf.file().display()))
                    .send()
                    .await
                    .map_err(|e| SysinspectError::MasterGeneralError(format!("Request failed: {e}")))?;
                if resp.status() != reqwest::StatusCode::OK {
                    log::error!("Failed to download library {}: {}", lf.file().display(), resp.status());
                    continue;
                }
                let buff = resp.bytes().await.map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to read response: {e}")))?;
                let dst = self.cfg.sharelib_dir().join(DEFAULT_MODULES_LIB_DIR).join(lf.file());
                //let dst = self.cfg.sharelib_dir().join(lf.file());

                log::debug!("Writing library to {} ({} bytes)", dst.display().to_string().bright_yellow(), buff.len());

                if let Some(dst) = dst.parent()
                    && !dst.exists()
                {
                    log::debug!("Creating directory {}", dst.display().to_string().bright_yellow());
                    std::fs::create_dir_all(dst)?;
                }

                fs::write(&dst, buff)?;
                fs::write(dst.with_extension(REPO_MOD_SHA256_EXT), lf.checksum())?;
            }

            synced += 1;
            if synced % (libt / 0xf).max(1) == 0 || synced == libt {
                log::warn!("{}% of library objects synced", (synced * 100) / libt);
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
        log::info!("Syncing {modt} modules");
        let mut synced = 0;
        if modt > 0 {
            log::warn!("{}% of modules synced", (synced * 100) / modt);
        }

        // Modules
        for (name, attrs) in ridx.modules() {
            let is_binary = attrs.mod_type().eq("binary");
            let path = format!(
                "http://{}/repo/{}/{}/{}/{}",
                self.cfg.fileserver(),
                if is_binary { "bin" } else { "script" },
                if is_binary { ostype } else { "any" },
                if is_binary { osarch } else { "noarch" },
                attrs.subpath()
            );
            let dst = self.cfg.sharelib_dir().join(DEFAULT_MODULES_DIR).join(attrs.subpath());
            let dst_cs = dst.with_extension(REPO_MOD_SHA256_EXT);

            let (verified, fsc) =
                self.verify_artefact_by_subpath(DEFAULT_MODULES_DIR, attrs.subpath(), attrs.checksum()).await.unwrap_or((false, None));
            if !verified {
                log::debug!("Downloading module {} from {}", name.bright_yellow(), path);
                let resp = reqwest::Client::new()
                    .get(&path)
                    .send()
                    .await
                    .map_err(|e| SysinspectError::MasterGeneralError(format!("Request failed: {e}")))?;
                if resp.status() != reqwest::StatusCode::OK {
                    log::error!("Failed to download module {}: {} on url {}", name, resp.status(), path);
                    continue;
                }
                let buff = resp.bytes().await.map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to read response: {e}")))?;

                // Check if we need to write that
                log::debug!("Writing module to {}", dst.display().to_string().bright_yellow());
                if let Some(pdst) = dst.parent()
                    && !pdst.exists()
                {
                    log::debug!("Creating directory {}", pdst.display().to_string().bright_yellow());
                    std::fs::create_dir_all(pdst)?;
                }
                fs::write(&dst, buff)?;

                // chmod +X
                let mut p = fs::metadata(&dst)?.permissions();
                p.set_mode(0o755);
                fs::set_permissions(&dst, p)?;
            }

            if let Some(fsc) = fsc
                && (!dst_cs.exists() || !verified)
            {
                log::debug!("Updating module checksum as {}", dst_cs.display().to_string().bright_yellow());
                fs::write(dst_cs, fsc)?;
            }
            synced += 1;
            if synced % (modt / 0xf).max(1) == 0 || synced == modt {
                log::warn!("{}% of modules synced", (synced * 100) / modt);
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
    /// Format a library path for `module -Ll` with runtime-aware filename colors.
    pub(crate) fn format_library_name(name: &str) -> String {
        for marker in ["site-lua/", "site-packages/"] {
            if let Some((prefix, suffix)) = name.split_once(marker) {
                return format!("{}{}{}", prefix.white(), marker.white(), suffix.yellow());
            }
        }

        let (prefix, file) = match name.rsplit_once('/') {
            Some((prefix, file)) => (format!("{prefix}/"), file),
            None => (String::new(), name),
        };

        let file = match file.rsplit_once('.') {
            Some((_, "lua")) => file.bright_cyan().to_string(),
            Some((_, "py")) => file.bright_blue().to_string(),
            Some((_, "wasm")) => file.bright_magenta().to_string(),
            _ => file.bright_white().to_string(),
        };

        format!("{}{}", prefix.bright_white().bold(), file)
    }

    fn pad_visible(text: &str, width: usize) -> String {
        let visible = ANSI_ESCAPE_RE.replace_all(text, "").chars().count();
        if visible >= width { text.to_string() } else { format!("{text}{}", " ".repeat(width - visible)) }
    }

    /// Creates a new ModPakRepo with the given root path.
    pub fn new(root: PathBuf) -> Result<Self, SysinspectError> {
        if !root.exists() {
            log::info!("Creating module repository at {}", root.display());
            std::fs::create_dir_all(&root)?;
            fs::write(root.join(REPO_MOD_INDEX), ModPakRepoIndex::new().to_yaml()?)?; // XXX: needs flock
        }

        let ridx = root.join(REPO_MOD_INDEX);
        if !ridx.exists() {
            log::info!("Creating module repository index at {}", ridx.display());
            fs::write(&ridx, ModPakRepoIndex::new().to_yaml()?)?;
        }

        Ok(Self { root: root.clone(), idx: ModPakRepoIndex::from_yaml(&fs::read_to_string(ridx)?)? })
    }

    /// Parses an object file and returns its architecture and OS ABI.
    fn parse_obj(buff: &[u8]) -> Result<(bool, &str, &str), SysinspectError> {
        match Object::parse(buff).map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to parse object: {e}")))? {
            Object::Elf(elf) => {
                let arch = match elf.header.e_machine {
                    goblin::elf::header::EM_X86_64 => "x86_64",
                    goblin::elf::header::EM_ARM => "ARM",
                    goblin::elf::header::EM_AARCH64 => "ARM64",
                    goblin::elf::header::EM_RISCV => "RISC-V",
                    _ => return Err(SysinspectError::MasterGeneralError("Unsupported ELF arch".to_string())),
                };
                Ok((true, arch, Self::get_os_label(&elf)))
            }
            _ => Ok((false, "noarch", "any")),
        }
    }

    /// Heuristic to determine the OS label of an ELF file, since EI_OSABI is often unreliable.
    fn get_os_label(elf: &goblin::elf::Elf) -> &'static str {
        // Check section names - BSDs put their identity here
        for sh in &elf.section_headers {
            if let Some(name) = elf.shdr_strtab.get_at(sh.sh_name) {
                if name.contains("netbsd") {
                    return "netbsd";
                }
                if name.contains("freebsd") {
                    return "freebsd";
                }
                if name.contains("openbsd") {
                    return "openbsd";
                }
            }
        }

        // Fallback: check EI_OSABI (works for Linux, sometimes)
        let osabi = elf.header.e_ident[goblin::elf::header::EI_OSABI];
        match osabi {
            goblin::elf::header::ELFOSABI_LINUX => "linux",
            goblin::elf::header::ELFOSABI_FREEBSD => "freebsd",
            goblin::elf::header::ELFOSABI_NETBSD => "netbsd",
            goblin::elf::header::ELFOSABI_OPENBSD => "openbsd",
            _ => "linux", // Default assumption
        }
    }

    /// Adds a library to the repository.
    pub fn add_library(&mut self, p: PathBuf) -> Result<(), SysinspectError> {
        let path = self.root.join("lib");
        if !path.exists() {
            log::info!("Creating module repository at {}", path.display());
            std::fs::create_dir_all(&path)?;
        }

        let mut options = CopyOptions::new();
        options.overwrite = true; // Overwrite existing files if necessary
        options.copy_inside = true; // Copy the contents inside `p` instead of the directory itself
        options.content_only = true; // Copy only the contents of the directory

        log::info!("Copying library from {} to {}", p.display(), path.display());
        fs_extra::dir::copy(&p, &path, &options).map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to copy library: {e}")))?;
        self.idx.index_library(&path)?;
        log::debug!("Writing index to {}", self.root.join(REPO_MOD_INDEX).display().to_string().bright_yellow());
        fs::write(self.root.join(REPO_MOD_INDEX), self.idx.to_yaml()?)?; // XXX: needs flock
        log::info!("Library {} added to index", p.display().to_string().bright_yellow());
        Ok(())
    }

    /// Add an existing binary module.
    pub fn add_module(&mut self, mut meta: ModPakMetadata) -> Result<(), SysinspectError> {
        let path = fs::canonicalize(meta.get_path())?;
        log::info!("Adding module {}", path.display().to_string().bright_yellow());

        let buff = fs::read(path)?;
        let (is_bin, arch, p) = match Self::parse_obj(&buff) {
            Ok((true, arch, osabi)) => (true, arch.to_string(), osabi.to_string()),
            Ok((false, _, _)) => (false, "noarch".to_string(), "any".to_string()),
            Err(e) => return Err(e),
        };

        meta.set_arch(&arch);

        log::info!("Platform: {p}");
        let module_subpath = meta.get_subpath();
        let subpath = PathBuf::from(format!("{}/{}/{}", if is_bin { "bin" } else { "script" }, p, arch)).join(&module_subpath);
        log::debug!("Subpath: {}", subpath.display().to_string().bright_yellow());
        if let Some(p) = self.root.join(&subpath).parent()
            && !p.exists()
        {
            log::debug!("Creating directory {}", p.display().to_string().bright_yellow());
            std::fs::create_dir_all(p).unwrap();
        }
        log::debug!("Copying module to {}", self.root.join(&subpath).display().to_string().bright_yellow());
        std::fs::copy(meta.get_path(), self.root.join(&subpath))?;
        let checksum = get_file_sha256(self.root.join(&subpath))?;

        // Quite ugly amount of args :-(
        self.idx
            .index_module(
                meta.get_name().as_str(),
                module_subpath.to_str().unwrap_or_default(),
                &p,
                &arch,
                meta.get_descr(),
                subpath.to_str().unwrap_or_default().starts_with("bin/"),
                &checksum,
                if meta.get_args().is_empty() { None } else { Some(meta.get_args().clone()) },
                if meta.get_opts().is_empty() { None } else { Some(meta.get_opts().clone()) },
            )
            .map_err(|e| SysinspectError::MasterGeneralError(format!("Failed to add module to index: {e}")))?;
        log::debug!("Writing index to {}", self.root.join(REPO_MOD_INDEX).display().to_string().bright_yellow());

        fs::write(self.root.join(REPO_MOD_INDEX), self.idx.to_yaml()?)?; // XXX: needs flock

        log::debug!("Module {} added to index", meta.get_name().bright_yellow());
        log::info!("Module {} added successfully", meta.get_name().bright_yellow());

        Ok(())
    }

    fn print_table(modules: &IndexMap<String, ModAttrs>, _verbose: bool) {
        let mut t = Table::new();
        t.set_format(*format::consts::FORMAT_CLEAN);
        /*
        t.set_titles(Row::new(vec![
            Cell::new("  "),
            Cell::new(&"Name".bright_yellow().bold().to_string()),
            Cell::new(&"Description".bright_white().bold().to_string()),
        ]));
        */

        // Sort modules by name
        let mut entries: Vec<(&String, &ModAttrs)> = modules.iter().collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        let wrap_opts = Options::new(50);
        for (modname, modattr) in entries {
            let lines = wrap(modattr.descr(), &wrap_opts);
            if lines.is_empty() {
                t.add_row(Row::new(vec![Cell::new("  "), Cell::new(&modname.bright_yellow().to_string()), Cell::new("")]));
                continue;
            }

            t.add_row(Row::new(vec![Cell::new("  "), Cell::new(&modname.bright_yellow().to_string()), Cell::new(&lines[0])]));
            for l in lines.iter().skip(1) {
                t.add_row(Row::new(vec![Cell::new(""), Cell::new(""), Cell::new(l)]));
            }
            if lines.len() > 1 {
                t.add_row(Row::new(vec![Cell::new(""), Cell::new(""), Cell::new("")]));
            }
        }

        t.printstd();
    }

    /// Lists all libraries in the repository.
    pub fn list_libraries(&self, expr: Option<&str>) -> Result<(), SysinspectError> {
        let expr = glob::Pattern::new(expr.unwrap_or("*")).map_err(|e| SysinspectError::MasterGeneralError(format!("Invalid pattern: {e}")))?;
        let mut rows = Vec::<(String, String, String, String, String)>::new();
        for (_, mpklf) in self.idx.library() {
            if !expr.matches(&mpklf.file().display().to_string()) {
                continue;
            }

            let buff = match fs::read(self.root.join(PathBuf::from(DEFAULT_MODULES_LIB_DIR)).join(mpklf.file())) {
                Ok(data) => data,
                Err(e) => {
                    log::error!("Failed to read library file {}: {}", mpklf.file().display(), e);
                    continue;
                }
            };

            let (kind, arch, p) = if mpklf.kind() == "binary" {
                match Self::parse_obj(&buff) {
                    Ok((true, arch, osabi)) => ("binary".to_string(), arch.to_string(), osabi.to_string()),
                    Ok((false, _, _)) => ("binary".to_string(), "noarch".to_string(), "any".to_string()),
                    Err(e) => {
                        log::error!("Failed to parse object for {}: {}", mpklf.file().display(), e);
                        continue;
                    }
                }
            } else {
                (mpklf.kind().to_string(), "noarch".to_string(), "any".to_string())
            };

            rows.push((
                match kind.as_str() {
                    "wasm" | "binary" => "binary".to_string(),
                    _ => "script".to_string(),
                },
                mpklf.file().display().to_string(),
                p.to_title_case(),
                arch,
                format!("{}...{}", &mpklf.checksum()[..4], &mpklf.checksum()[mpklf.checksum().len() - 4..]),
            ));
        }

        if rows.is_empty() {
            log::warn!("No libraries found matching the pattern: {}", expr.as_str().bright_yellow());
            return Ok(());
        }

        let type_width = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(4).max("Type".chars().count());
        let name_width = rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(4).max("Name".chars().count());
        let os_width = rows.iter().map(|r| r.2.chars().count()).max().unwrap_or(2).max("OS".chars().count());
        let arch_width = rows.iter().map(|r| r.3.chars().count()).max().unwrap_or(4).max("Arch".chars().count());
        let sha_width = rows.iter().map(|r| r.4.chars().count()).max().unwrap_or(6).max("SHA256".chars().count());

        println!(
            "{}  {}  {}  {}  {}",
            Self::pad_visible(&"Type".bright_yellow().to_string(), type_width),
            Self::pad_visible(&"Name".bright_yellow().to_string(), name_width),
            Self::pad_visible(&"OS".bright_yellow().to_string(), os_width),
            Self::pad_visible(&"Arch".bright_yellow().to_string(), arch_width),
            Self::pad_visible(&"SHA256".bright_yellow().to_string(), sha_width),
        );
        println!(
            "{}  {}  {}  {}  {}",
            "─".repeat(type_width),
            "─".repeat(name_width),
            "─".repeat(os_width),
            "─".repeat(arch_width),
            "─".repeat(sha_width),
        );

        for (kind, name, os_name, arch, sha) in rows {
            println!(
                "{}  {}  {}  {}  {}",
                Self::pad_visible(&kind.bright_green().to_string(), type_width),
                Self::pad_visible(&Self::format_library_name(&name), name_width),
                Self::pad_visible(&os_name.bright_green().to_string(), os_width),
                Self::pad_visible(&arch.bright_green().to_string(), arch_width),
                Self::pad_visible(&sha.green().to_string(), sha_width),
            );
        }

        Ok(())
    }

    pub fn module_info(&self, name: &str) -> Result<(), SysinspectError> {
        let mut found = false;
        for (p, archset) in self.idx.all_modules(None, Some(vec![name])) {
            let p = if p.eq("sysv") { "Linux" } else { p.as_str() };
            for (arch, modules) in archset {
                println!("{} ({}): ", p, arch.bright_green());
                Self::print_table(&modules, true);
                found = true;
            }
        }

        if !found {
            log::warn!("No module found matching the name: {}", name.bright_yellow());
        }

        Ok(())
    }

    /// Lists all modules in the repository.
    pub fn list_modules(&self) -> Result<(), SysinspectError> {
        let osn = HashMap::from([
            ("sysv", "Linux"),
            ("any", "Any"),
            ("linux", "Linux"),
            ("netbsd", "NetBSD"),
            ("freebsd", "FreeBSD"),
            ("openbsd", "OpenBSD"),
        ]);

        let allmods = self.idx.all_modules(None, None);
        let mut platforms = allmods.iter().map(|(p, _)| p.to_string()).collect::<Vec<_>>();
        platforms.sort();

        for p in platforms {
            let archset = allmods.get(&p).unwrap(); // safe: iter above
            let p = if osn.contains_key(p.as_str()) { osn.get(p.as_str()).unwrap() } else { p.as_str() };
            for (arch, modules) in archset {
                println!("{} ({}): ", p, arch.bright_green());
                Self::print_table(modules, false);
            }
        }
        Ok(())
    }

    /// Resolves library removal expressions to concrete library names.
    ///
    /// Args:
    /// * `names` - Exact names or glob expressions such as `library/*`.
    ///
    /// Returns:
    /// * `Ok(Vec<String>)` containing sorted unique library names present in the index.
    /// * `Err(SysinspectError)` if any expression is an invalid glob pattern.
    fn resolve_library_names(&self, names: Vec<String>) -> Result<Vec<String>, SysinspectError> {
        let mut resolved = IndexMap::<String, ()>::new();

        for name in names {
            let expr = glob::Pattern::new(&name).map_err(|e| SysinspectError::MasterGeneralError(format!("Invalid pattern '{name}': {e}")))?;
            for lib in self.idx.library().keys().filter(|lib| expr.matches(lib) || lib.strip_prefix("lib/").is_some_and(|rel| expr.matches(rel))) {
                resolved.insert(lib.to_string(), ());
            }
        }

        let mut names = resolved.into_keys().collect::<Vec<_>>();
        names.sort();
        Ok(names)
    }

    /// Resolves module removal expressions to concrete module names.
    ///
    /// Args:
    /// * `names` - Exact names or glob expressions such as `cfg.*` or `*`.
    ///
    /// Returns:
    /// * `Ok(Vec<String>)` containing sorted unique module names present in the index.
    /// * `Err(SysinspectError)` if any expression is an invalid glob pattern.
    fn resolve_module_names(&self, names: Vec<&str>) -> Result<Vec<String>, SysinspectError> {
        let mut known = IndexMap::<String, ()>::new();
        for (_, archset) in self.idx.all_modules(None, None) {
            for (_, modules) in archset {
                for name in modules.keys() {
                    known.insert(name.to_string(), ());
                }
            }
        }

        let mut resolved = IndexMap::<String, ()>::new();
        for name in names {
            let expr = glob::Pattern::new(name).map_err(|e| SysinspectError::MasterGeneralError(format!("Invalid pattern '{name}': {e}")))?;
            for module in known.keys().filter(|module| expr.matches(module)) {
                resolved.insert(module.to_string(), ());
            }
        }

        let mut names = resolved.into_keys().collect::<Vec<_>>();
        names.sort();
        Ok(names)
    }

    /// Removes libraries from the repository and index.
    ///
    /// Args:
    /// * `names` - Exact library names or glob expressions.
    ///
    /// Returns:
    /// * `Ok(())` after removing all matching libraries and rewriting the index.
    /// * `Err(SysinspectError)` if a glob expression is invalid or the index cannot be updated.
    pub fn remove_library(&mut self, names: Vec<String>) -> Result<(), SysinspectError> {
        let names = self.resolve_library_names(names)?;
        let mut c = 0;
        log::info!("Removing {} librar{}", names.len(), if names.len() > 1 { "ies" } else { "y" });
        for subp in names {
            self.root.join(PathBuf::from(DEFAULT_MODULES_LIB_DIR).join(&subp)).exists().then(|| {
                fs::remove_file(self.root.join(PathBuf::from(DEFAULT_MODULES_LIB_DIR).join(&subp))).unwrap_or_else(|err| {
                    log::error!("Failed to remove library {}: {}", subp.bright_yellow(), err);
                });
            });
            self.idx.remove_library(&subp)?;
            log::info!("{} has been removed", subp.bright_yellow());
            c += 1;
        }

        if c > 0 {
            fs::write(self.root.join(REPO_MOD_INDEX), self.idx.to_yaml()?)?; // XXX: needs flock
            log::info!("{} librar{} removed", c, if c > 1 { "ies" } else { "y" });
        } else {
            log::error!("No libraries found to remove");
        }

        Ok(())
    }

    /// Removes a module from the repository and index.
    pub fn remove_module(&mut self, name: Vec<&str>) -> Result<(), SysinspectError> {
        let names = self.resolve_module_names(name)?;
        let mut c = 0;
        let name_refs = names.iter().map(String::as_str).collect::<Vec<_>>();
        for (p, archset) in self.idx.all_modules(None, Some(name_refs.clone())) {
            for (arch, modules) in archset {
                for attrs in modules.values() {
                    let path = self.root.join(
                        PathBuf::from(format!("{}/{}/{}", if attrs.mod_type().eq("binary") { "bin" } else { "script" }, p, arch))
                            .join(attrs.subpath()),
                    );
                    if path.exists() {
                        if let Err(err) = fs::remove_file(&path) {
                            log::error!("Failed to remove module: {err}");
                        }

                        // Also remove the whole directory if it's empty already
                        if let Some(p) = path.parent()
                            && let Ok(entries) = fs::read_dir(p)
                            && entries.count() == 0
                        {
                            fs::remove_dir(p)?;
                        }
                        c += 1;
                    } else {
                        log::error!("Module not found at {}", path.display().to_string().bright_yellow());
                    }
                }
            }
        }

        // Update the index
        if c > 0 {
            self.idx.remove_module_all(name_refs)?; // unindex
            fs::write(self.root.join(REPO_MOD_INDEX), self.idx.to_yaml()?)?; // XXX: needs flock
            log::info!("Module{} {} has been removed", if names.len() > 1 { "s" } else { "" }, names.join(", ").bright_yellow());
        } else {
            log::error!("No module{} found: {}", if names.len() > 1 { "s" } else { "" }, names.join(", ").bright_yellow());
        }

        Ok(())
    }
}
