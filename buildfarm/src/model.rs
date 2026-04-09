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
            os: "local".to_string(),
            arch: "local".to_string(),
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
        format!("{} {} {}", self.os(), self.arch(), self.destination())
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
