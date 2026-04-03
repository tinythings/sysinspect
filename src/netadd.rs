//! CLI-facing planning types for `sysinspect network --add`.

use clap::ArgMatches;
use colored::Colorize;
use libcommon::SysinspectError;
use libsysinspect::util::pad_visible;
use std::{
    collections::BTreeSet,
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

/// One validated onboarding request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddRequest {
    pub(crate) hosts: Vec<HostSpec>,
    pub(crate) user: String,
}

/// One fully resolved onboarding target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddHost {
    pub(crate) host: String,
    pub(crate) user: String,
    pub(crate) path: Option<String>,
}

/// One validated onboarding plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddPlan {
    pub(crate) items: Vec<AddHost>,
}

/// One operator-visible onboarding outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddOutcome {
    pub(crate) host: AddHost,
    pub(crate) state: &'static str,
    pub(crate) detail: String,
}

/// Dedicated Phase-2 onboarding workflow entrypoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NetworkAddWorkflow {
    req: AddRequest,
}

impl NetworkAddWorkflow {
    /// Build one workflow from CLI matches.
    pub(crate) fn from_matches(am: &ArgMatches) -> Result<Self, SysinspectError> {
        Ok(Self { req: parse_request(am)? })
    }

    /// Validate and resolve the host batch.
    pub(crate) fn plan(&self) -> Result<AddPlan, SysinspectError> {
        let mut seen = BTreeSet::<(String, String, String)>::new();
        let mut items = Vec::<AddHost>::new();

        for spec in &self.req.hosts {
            let item = AddHost { host: spec.host.clone(), user: spec.user.clone().unwrap_or_else(|| self.req.user.clone()), path: spec.path.clone() };
            let key = (item.user.clone(), item.host.to_ascii_lowercase(), item.path.clone().unwrap_or_default());
            if !seen.insert(key) {
                return Err(SysinspectError::InvalidQuery(format!(
                    "Duplicate host entry after normalisation: {}",
                    display_host(&item).bright_yellow()
                )));
            }
            items.push(item);
        }

        items.sort_by(|a, b| (&a.host, &a.user, &a.path).cmp(&(&b.host, &b.user, &b.path)));
        Ok(AddPlan { items })
    }

    /// Produce the Phase-2 console view.
    pub(crate) fn render(&self) -> Result<String, SysinspectError> {
        let plan = self.plan()?;
        let rows = plan.items.iter().cloned().map(|host| AddOutcome { detail: "validated".to_string(), host, state: "planned" }).collect::<Vec<_>>();
        Ok(render_outcomes(&rows))
    }
}

/// Parse and resolve one `network --add` request.
pub(crate) fn parse(am: &ArgMatches) -> Result<AddPlan, SysinspectError> {
    NetworkAddWorkflow::from_matches(am)?.plan()
}

fn parse_request(am: &ArgMatches) -> Result<AddRequest, SysinspectError> {
    if !am.get_flag("add") {
        return Err(SysinspectError::InvalidQuery("Host onboarding requires --add".to_string()));
    }
    if am.get_one::<String>("query-pos").is_some_and(|v| v != "*") {
        return Err(SysinspectError::InvalidQuery("Host onboarding does not accept positional selectors".to_string()));
    }

    let hosts = if let Some(v) = am.get_one::<String>("hostnames") {
        parse_hostnames(v)?
    } else if let Some(p) = am.get_one::<String>("list") {
        parse_list(Path::new(p))?
    } else {
        return Err(SysinspectError::InvalidQuery("Specify either --hostnames or --list for --add".to_string()));
    };
    if hosts.is_empty() {
        return Err(SysinspectError::InvalidQuery("No host entries were supplied for --add".to_string()));
    }

    Ok(AddRequest {
        hosts,
        user: am
            .get_one::<String>("user")
            .cloned()
            .or_else(current_user)
            .ok_or_else(|| SysinspectError::ConfigError("No default SSH user could be resolved".to_string()))?,
    })
}

fn render_outcomes(rows: &[AddOutcome]) -> String {
    let widths = (
        rows.iter().map(|row| row.host.host.chars().count()).max().unwrap_or(4).max("HOST".chars().count()),
        rows.iter().map(|row| row.host.user.chars().count()).max().unwrap_or(4).max("USER".chars().count()),
        rows.iter().map(|row| row.host.path.as_deref().unwrap_or(DEFAULT_DEST).chars().count()).max().unwrap_or(4).max("PATH".chars().count()),
        rows.iter().map(|row| row.state.chars().count()).max().unwrap_or(5).max("STATE".chars().count()),
        rows.iter().map(|row| row.detail.chars().count()).max().unwrap_or(6).max("DETAIL".chars().count()),
    );
    let mut out = vec![
        format!(
            "{}  {}  {}  {}  {}",
            pad_visible(&"HOST".yellow().to_string(), widths.0),
            pad_visible(&"USER".yellow().to_string(), widths.1),
            pad_visible(&"PATH".yellow().to_string(), widths.2),
            pad_visible(&"STATE".yellow().to_string(), widths.3),
            pad_visible(&"DETAIL".yellow().to_string(), widths.4),
        ),
        format!("{}  {}  {}  {}  {}", "─".repeat(widths.0), "─".repeat(widths.1), "─".repeat(widths.2), "─".repeat(widths.3), "─".repeat(widths.4),),
    ];

    for row in rows {
        out.push(format!(
            "{}  {}  {}  {}  {}",
            pad_visible(&row.host.host.bright_green().to_string(), widths.0),
            pad_visible(&row.host.user.bright_blue().to_string(), widths.1),
            pad_visible(row.host.path.as_deref().unwrap_or(DEFAULT_DEST), widths.2),
            pad_visible(&row.state.bright_cyan().to_string(), widths.3),
            pad_visible(&row.detail, widths.4),
        ));
    }

    out.push(String::new());
    out.push(format!("Planned onboarding for {} host{}", rows.len(), if rows.len() == 1 { "" } else { "s" }));
    out.join("\n")
}

fn parse_hostnames(value: &str) -> Result<Vec<HostSpec>, SysinspectError> {
    value.split(',').map(str::trim).filter(|v| !v.is_empty()).map(parse_entry).collect()
}

fn parse_list(path: &Path) -> Result<Vec<HostSpec>, SysinspectError> {
    fs::read_to_string(path)?.lines().map(str::trim).filter(|v| !v.is_empty() && !v.starts_with('#')).map(parse_entry).collect()
}

fn parse_entry(raw: &str) -> Result<HostSpec, SysinspectError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(SysinspectError::InvalidQuery("Empty host entry is not allowed".to_string()));
    }

    let (user, rest) = match raw.split_once('@') {
        Some((user, rest)) if user.is_empty() || rest.is_empty() || rest.contains('@') => {
            return Err(SysinspectError::InvalidQuery(format!("Invalid host entry: {raw}")));
        }
        Some((user, rest)) => (Some(user.to_string()), rest),
        None => (None, raw),
    };
    let (host, path) = match rest.split_once(':') {
        Some((host, path)) if host.is_empty() || path.is_empty() => {
            return Err(SysinspectError::InvalidQuery(format!("Invalid host entry: {raw}")));
        }
        Some((host, _)) if rest[host.len() + 1..].contains(':') => {
            return Err(SysinspectError::InvalidQuery(format!("Host entry contains too many ':' separators: {raw}")));
        }
        Some((host, path)) => (host.to_string(), Some(path.to_string())),
        None => (rest.to_string(), None),
    };
    if host.trim().is_empty() {
        return Err(SysinspectError::InvalidQuery(format!("Invalid host entry: {raw}")));
    }

    Ok(HostSpec { user, host, path })
}

fn current_user() -> Option<String> {
    ["USER", "LOGNAME", "USERNAME"].into_iter().find_map(|k| env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()))
}

fn display_host(host: &AddHost) -> String {
    format!("{}@{}{}", host.user, host.host, host.path.as_ref().map(|path| format!(":{path}")).unwrap_or_default())
}

/// Resolve one relative remote path under a known remote home directory.
pub(crate) fn resolve_remote_path(home: &Path, path: Option<&str>) -> Option<PathBuf> {
    match path.map(str::trim).filter(|v| !v.is_empty()) {
        Some(path) if path.starts_with('/') => Some(PathBuf::from(path)),
        Some(path) => Some(home.join(path)),
        None => None,
    }
}
