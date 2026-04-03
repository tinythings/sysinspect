use crate::sshprobe::detect::{CpuArch, PlatformFamily, ProbeInfo};
use libcommon::SysinspectError;
use libmodpak::SysInspectModPak;
use std::{collections::BTreeMap, path::PathBuf};

/// Supported minion artefact OS families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ArtifactFamily {
    Linux,
    FreeBsd,
    NetBsd,
    OpenBsd,
    Qnx,
}

/// Supported minion artefact CPU architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ArtifactArch {
    X86_64,
    Aarch64,
    Arm,
    RiscV64,
}

/// Stable platform selector used by the minion artefact catalogue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PlatformId {
    pub(crate) family: ArtifactFamily,
    pub(crate) arch: ArtifactArch,
    pub(crate) abi: Option<String>,
}

/// One selectable sysminion artefact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MinionArtifact {
    pub(crate) platform: PlatformId,
    pub(crate) version: String,
    pub(crate) checksum: String,
    pub(crate) path: PathBuf,
    pub(crate) source: String,
}

/// Reusable minion artefact catalogue backed by `mod.index`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MinionCatalogue {
    pub(crate) root: PathBuf,
    items: BTreeMap<PlatformId, MinionArtifact>,
}

impl ArtifactFamily {
    fn as_repo(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::FreeBsd => "freebsd",
            Self::NetBsd => "netbsd",
            Self::OpenBsd => "openbsd",
            Self::Qnx => "qnx",
        }
    }

    fn from_probe(family: PlatformFamily) -> Result<Self, SysinspectError> {
        match family {
            PlatformFamily::Linux => Ok(Self::Linux),
            PlatformFamily::FreeBsd => Ok(Self::FreeBsd),
            PlatformFamily::NetBsd => Ok(Self::NetBsd),
            PlatformFamily::OpenBsd => Ok(Self::OpenBsd),
            PlatformFamily::Qnx => Ok(Self::Qnx),
            PlatformFamily::Unknown => Err(SysinspectError::InvalidQuery("Unsupported target platform: unknown".to_string())),
        }
    }

    fn from_repo(value: &str) -> Option<Self> {
        match value {
            "linux" => Some(Self::Linux),
            "freebsd" => Some(Self::FreeBsd),
            "netbsd" => Some(Self::NetBsd),
            "openbsd" => Some(Self::OpenBsd),
            "qnx" => Some(Self::Qnx),
            _ => None,
        }
    }
}

impl ArtifactArch {
    fn as_repo(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "arm64",
            Self::Arm => "arm",
            Self::RiscV64 => "riscv",
        }
    }

    fn from_probe(arch: CpuArch) -> Result<Self, SysinspectError> {
        match arch {
            CpuArch::X86_64 => Ok(Self::X86_64),
            CpuArch::Aarch64 => Ok(Self::Aarch64),
            CpuArch::Arm => Ok(Self::Arm),
            CpuArch::RiscV64 => Ok(Self::RiscV64),
            CpuArch::X86 | CpuArch::Ppc64Le | CpuArch::Unknown => {
                Err(SysinspectError::InvalidQuery(format!("Unsupported target architecture: {:?}", arch).to_lowercase()))
            }
        }
    }

    fn from_repo(value: &str) -> Option<Self> {
        match value {
            "x86_64" => Some(Self::X86_64),
            "arm64" => Some(Self::Aarch64),
            "arm" => Some(Self::Arm),
            "riscv" => Some(Self::RiscV64),
            _ => None,
        }
    }
}

impl PlatformId {
    /// Build one artefact selector from probed host data.
    pub(crate) fn from_probe(info: &ProbeInfo) -> Result<Self, SysinspectError> {
        Ok(Self { family: ArtifactFamily::from_probe(info.family)?, arch: ArtifactArch::from_probe(info.arch)?, abi: None })
    }

    fn display(&self) -> String {
        format!("{}/{}", self.family.as_repo(), self.arch.as_repo())
    }
}

impl MinionCatalogue {
    /// Open the registered minion artefact catalogue from the module repository.
    pub(crate) fn open(root: impl Into<PathBuf>) -> Result<Self, SysinspectError> {
        let root = root.into();
        let repo = SysInspectModPak::new(root.clone())?;
        let mut items = BTreeMap::new();

        for item in repo.minion_builds() {
            let (Some(family), Some(arch)) = (ArtifactFamily::from_repo(item.platform()), ArtifactArch::from_repo(item.arch())) else {
                continue;
            };
            let platform = PlatformId { family, arch, abi: None };
            items.insert(
                platform.clone(),
                MinionArtifact {
                    source: item.path().display().to_string(),
                    checksum: item.checksum().to_string(),
                    version: item.version().to_string(),
                    path: item.path().to_path_buf(),
                    platform,
                },
            );
        }

        Ok(Self { root, items })
    }

    /// Select one sysminion artefact for a probed target platform.
    pub(crate) fn select(&self, platform: &PlatformId) -> Result<MinionArtifact, SysinspectError> {
        let item = self.items.get(platform).cloned().ok_or_else(|| {
            SysinspectError::InvalidQuery(format!(
                "Missing sysminion artefact for {} in {}. Register it with sysinspect module -A -t -p /path/to/sysminion",
                platform.display(),
                self.root.display()
            ))
        })?;
        if !item.path.exists() {
            return Err(SysinspectError::InvalidQuery(format!(
                "Registered sysminion artefact for {} is missing on disk: {}",
                platform.display(),
                item.path.display()
            )));
        }
        Ok(item)
    }
}
