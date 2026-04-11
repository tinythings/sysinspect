use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetMode {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTarget {
    os: String,
    arch: String,
    destination: String,
    mode: TargetMode,
}

impl BuildTarget {
    pub fn local() -> Self {
        Self {
            os: Self::local_os(),
            arch: Self::local_arch(),
            destination: "local".to_string(),
            mode: TargetMode::Local,
        }
    }

    pub fn remote(os: &str, arch: &str, destination: &str) -> Self {
        Self {
            os: os.to_string(),
            arch: arch.to_string(),
            destination: destination.to_string(),
            mode: TargetMode::Remote,
        }
    }

    pub fn os(&self) -> &str {
        &self.os
    }

    pub fn arch(&self) -> &str {
        &self.arch
    }

    pub fn destination(&self) -> &str {
        &self.destination
    }

    pub fn mode(&self) -> &TargetMode {
        &self.mode
    }

    pub fn is_local(&self) -> bool {
        matches!(self.mode, TargetMode::Local)
    }

    pub fn host(&self) -> &str {
        self.destination
            .split_once(':')
            .map(|(host, _)| host)
            .unwrap_or(self.destination())
    }

    pub fn remote_path(&self) -> &str {
        self.destination
            .split_once(':')
            .map(|(_, path)| path)
            .unwrap_or(self.destination())
    }

    pub fn make_cmd(&self) -> &str {
        match self.os() {
            "FreeBSD" => "gmake",
            _ => "make",
        }
    }

    pub fn log_key(&self) -> String {
        self.destination()
            .chars()
            .map(|ch| if ch == '/' || ch == ':' || ch == '@' { '_' } else { ch })
            .collect()
    }

    pub fn title(&self) -> String {
        self.is_local()
            .then_some(format!("{} {} localhost", self.os(), self.arch()))
            .unwrap_or_else(|| format!("{} {} {}", self.os(), self.arch(), self.destination()))
    }

    pub fn os_label(&self) -> String {
        self.is_linux_family()
            .then_some(self.linux_os_label())
            .unwrap_or_else(|| self.bsdish_os_label())
    }

    pub fn mirror_directory(&self, mirror_root: &Path) -> PathBuf {
        mirror_root.join(self.os_label_dir())
    }

    fn os_label_dir(&self) -> String {
        self.os_label()
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '.' {
                    ch.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string()
    }

    fn is_linux_family(&self) -> bool {
        matches!(self.os(), "GNU/Linux" | "Linux")
            || self.os().starts_with("GNU/Linux_")
            || self.os().starts_with("Linux_")
            || self.os().starts_with("GNU/Linux-")
            || self.os().starts_with("Linux-")
    }

    fn linux_os_label(&self) -> String {
        self.os()
            .split_once(['_', '-'])
            .map(|(_, suffix)| suffix)
            .and_then(Self::linux_label_from_suffix)
            .unwrap_or_else(|| "Linux".to_string())
    }

    fn linux_label_from_suffix(suffix: &str) -> Option<String> {
        let mut parts = suffix.split('-');
        let kernel = parts.next().filter(|part| !part.is_empty())?;
        let libc = parts.next().filter(|part| !part.is_empty())?;

        Some(format!("Linux {} {}", kernel, libc.replace('_', " ")))
    }

    fn bsdish_os_label(&self) -> String {
        self.os()
            .split_once(['_', '-'])
            .map(|(family, suffix)| format!("{family} {suffix}"))
            .unwrap_or_else(|| self.os().to_string())
    }

    fn local_os() -> String {
        match std::env::consts::OS {
            "linux" => "GNU/Linux".to_string(),
            "freebsd" => "FreeBSD".to_string(),
            "netbsd" => "NetBSD".to_string(),
            "openbsd" => "OpenBSD".to_string(),
            other => other.to_string(),
        }
    }

    fn local_arch() -> String {
        match std::env::consts::ARCH {
            "x86_64" => "x86_64".to_string(),
            "aarch64" => "aarch64".to_string(),
            other => other.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirroredResultLayout {
    entry: String,
    roots: Vec<PathBuf>,
}

impl MirroredResultLayout {
    pub fn for_entry(entry: &str) -> Self {
        Self {
            entry: entry.to_string(),
            roots: Self::known_roots(entry),
        }
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    fn known_roots(entry: &str) -> Vec<PathBuf> {
        match entry {
            "dev" | "all-dev" | "release" | "all" | "modules-dev" | "modules" => vec![PathBuf::from("build/stage")],
            "modules-dist-dev" => vec![PathBuf::from("build/stage"), PathBuf::from("build/modules-dist")],
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultMirrorPlan {
    enabled: bool,
    root: PathBuf,
    layout: MirroredResultLayout,
}

impl ResultMirrorPlan {
    pub fn new(enabled: bool, root: PathBuf, entry: &str) -> Self {
        Self {
            enabled,
            root,
            layout: MirroredResultLayout::for_entry(entry),
        }
    }

    pub fn disabled(root: PathBuf, entry: &str) -> Self {
        Self::new(false, root, entry)
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn layout(&self) -> &MirroredResultLayout {
        &self.layout
    }

    pub fn target_root(&self, target: &BuildTarget) -> PathBuf {
        target.mirror_directory(self.root())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildfarmConfig {
    targets: Vec<BuildTarget>,
}

impl BuildfarmConfig {
    pub fn parse(src: &str) -> Result<Self, String> {
        Self::from_lines(src.lines().enumerate().filter_map(Line::meaningful).map(Line::parse).collect::<Result<Vec<_>, _>>()?)
    }

    pub fn targets(&self) -> &[BuildTarget] {
        &self.targets
    }

    fn from_lines(targets: Vec<BuildTarget>) -> Result<Self, String> {
        (!targets.is_empty())
            .then_some(Self { targets })
            .ok_or_else(|| "buildfarm config has no targets".to_string())
    }
}

struct Line<'a> {
    lineno: usize,
    text: &'a str,
}

impl<'a> Line<'a> {
    fn meaningful((lineno, raw): (usize, &'a str)) -> Option<Self> {
        raw.trim()
            .is_empty()
            .then_some(None)
            .unwrap_or_else(|| raw.trim().starts_with('#').then_some(None).unwrap_or(Some(Self { lineno: lineno + 1, text: raw.trim() })))
    }

    fn parse(self) -> Result<BuildTarget, String> {
        (self.text == "local")
            .then_some(Ok(BuildTarget::local()))
            .unwrap_or_else(|| self.remote_target())
    }

    fn remote_target(&self) -> Result<BuildTarget, String> {
        self.fields()
            .and_then(|fields| {
                fields[2]
                    .contains(':')
                    .then_some(BuildTarget::remote(fields[0], fields[1], fields[2]))
                    .ok_or_else(|| format!("invalid buildfarm line {}: missing host:/destination in third field", self.lineno))
            })
    }

    fn fields(&self) -> Result<Vec<&str>, String> {
        self.text
            .split_whitespace()
            .collect::<Vec<_>>()
            .pipe_ref(|fields| {
                (fields.len() == 3)
                    .then_some(fields.clone())
                    .ok_or_else(|| format!("invalid buildfarm line {}: expected 3 fields, got {}", self.lineno, fields.len()))
            })
    }
}

trait PipeRef: Sized {
    fn pipe_ref<T>(self, f: impl FnOnce(&Self) -> T) -> T {
        f(&self)
    }
}

impl<T> PipeRef for T {}
