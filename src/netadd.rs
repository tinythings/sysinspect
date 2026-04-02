use clap::ArgMatches;
use colored::Colorize;
use libcommon::SysinspectError;
use libsysinspect::util::pad_visible;
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

const DEFAULT_DEST: &str = "<system>";

/// Parsed host onboarding input before defaults are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostSpec {
    user: Option<String>,
    host: String,
    path: Option<String>,
}

/// One fully resolved onboarding target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddHost {
    pub(crate) host: String,
    pub(crate) user: String,
    pub(crate) path: Option<String>,
}

/// One validated onboarding request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddPlan {
    pub(crate) items: Vec<AddHost>,
}

/// Parse and resolve one `network --add` request.
pub(crate) fn parse(am: &ArgMatches) -> Result<AddPlan, SysinspectError> {
    if !am.get_flag("add") {
        return Err(SysinspectError::InvalidQuery("Host onboarding requires --add".to_string()));
    }
    if am.get_one::<String>("query-pos").is_some_and(|v| v != "*") {
        return Err(SysinspectError::InvalidQuery("Host onboarding does not accept positional selectors".to_string()));
    }

    let specs = if let Some(v) = am.get_one::<String>("names") {
        parse_names(v)?
    } else if let Some(p) = am.get_one::<String>("list") {
        parse_list(Path::new(p))?
    } else {
        return Err(SysinspectError::InvalidQuery("Specify either --names or --list for --add".to_string()));
    };

    if specs.is_empty() {
        return Err(SysinspectError::InvalidQuery("No host entries were supplied for --add".to_string()));
    }

    let default_user = am
        .get_one::<String>("user")
        .cloned()
        .or_else(current_user)
        .ok_or_else(|| SysinspectError::ConfigError("No default SSH user could be resolved".to_string()))?;
    let mut items = BTreeMap::<(String, String, String), AddHost>::new();
    for spec in specs {
        let user = spec.user.clone().unwrap_or_else(|| default_user.clone());
        let item = AddHost { host: spec.host.clone(), user, path: spec.path.clone() };
        items.entry((item.user.clone(), item.host.to_ascii_lowercase(), item.path.clone().unwrap_or_default())).or_insert(item);
    }

    Ok(AddPlan { items: items.into_values().collect() })
}

/// Render one onboarding plan as a compact operator table.
pub(crate) fn render(plan: &AddPlan) -> String {
    let widths = (
        plan.items.iter().map(|item| item.host.chars().count()).max().unwrap_or(4).max("HOST".chars().count()),
        plan.items.iter().map(|item| item.user.chars().count()).max().unwrap_or(4).max("USER".chars().count()),
        plan.items.iter().map(|item| item.path.as_deref().unwrap_or(DEFAULT_DEST).chars().count()).max().unwrap_or(4).max("PATH".chars().count()),
    );
    let mut out = vec![
        format!(
            "{}  {}  {}",
            pad_visible(&"HOST".yellow().to_string(), widths.0),
            pad_visible(&"USER".yellow().to_string(), widths.1),
            pad_visible(&"PATH".yellow().to_string(), widths.2)
        ),
        format!("{}  {}  {}", "─".repeat(widths.0), "─".repeat(widths.1), "─".repeat(widths.2)),
    ];

    for item in &plan.items {
        out.push(format!(
            "{}  {}  {}",
            pad_visible(&item.host.bright_green().to_string(), widths.0),
            pad_visible(&item.user.bright_blue().to_string(), widths.1),
            pad_visible(item.path.as_deref().unwrap_or(DEFAULT_DEST), widths.2)
        ));
    }

    out.push(String::new());
    out.push(format!("Planned onboarding for {} host{}", plan.items.len(), if plan.items.len() == 1 { "" } else { "s" }));
    out.join("\n")
}

fn parse_names(value: &str) -> Result<Vec<HostSpec>, SysinspectError> {
    value.split(',').map(str::trim).filter(|entry| !entry.is_empty()).map(parse_entry).collect()
}

fn parse_list(path: &Path) -> Result<Vec<HostSpec>, SysinspectError> {
    fs::read_to_string(path)?.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).map(parse_entry).collect()
}

fn parse_entry(raw: &str) -> Result<HostSpec, SysinspectError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(SysinspectError::InvalidQuery("Empty host entry is not allowed".to_string()));
    }

    let (user, host_path) = match raw.split_once('@') {
        Some((user, rest)) => {
            if user.is_empty() || rest.is_empty() || rest.contains('@') {
                return Err(SysinspectError::InvalidQuery(format!("Invalid host entry: {raw}")));
            }
            (Some(user.to_string()), rest)
        }
        None => (None, raw),
    };

    let (host, path) = match host_path.split_once(':') {
        Some((host, path)) if host.is_empty() || path.is_empty() => {
            return Err(SysinspectError::InvalidQuery(format!("Invalid host entry: {raw}")));
        }
        Some((host, _)) if host_path[host.len() + 1..].contains(':') => {
            return Err(SysinspectError::InvalidQuery(format!("Host entry contains too many ':' separators: {raw}")));
        }
        Some((host, path)) => (host.to_string(), Some(path.to_string())),
        None => (host_path.to_string(), None),
    };

    if host.trim().is_empty() {
        return Err(SysinspectError::InvalidQuery(format!("Invalid host entry: {raw}")));
    }

    Ok(HostSpec { user, host, path })
}

fn current_user() -> Option<String> {
    ["USER", "LOGNAME", "USERNAME"].into_iter().find_map(|key| env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()))
}

/// Resolve one relative remote path under a known remote home directory.
pub(crate) fn resolve_remote_path(home: &Path, path: Option<&str>) -> Option<PathBuf> {
    match path.map(str::trim).filter(|v| !v.is_empty()) {
        Some(path) if path.starts_with('/') => Some(PathBuf::from(path)),
        Some(path) => Some(home.join(path)),
        None => None,
    }
}
